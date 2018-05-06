#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate image;
#[macro_use]
extern crate lazy_static;
extern crate libobs_sys as libobs;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tera;
extern crate tokio_core;
extern crate websocket;

mod macros;
mod obs;
mod art;
mod text;

use art::AlbumArtSourceDefinition;
use futures::{future, stream, Future, IntoFuture, Stream};
use futures::sync::oneshot;
use std::collections::BTreeMap;
use std::io;
use std::sync::{Arc, Mutex, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use text::NowPlayingSourceDefinition;
use tokio_core::reactor::{Core, Handle, Remote, Timeout};
use websocket::{ClientBuilder, OwnedMessage, WebSocketError};
use websocket::url::Url;

obs_declare_module!(
    GpmdpModule,
    "gpmdp",
    "Display information from GPMDP.",
    "Matthew Donoughe <mdonoughe@gmail.com>"
);

obs_module_use_default_locale!("en-US");

#[derive(Debug)]
struct GpmdpTrack {
    artist: Option<String>,
    album: Option<String>,
    title: Option<String>,
    album_art: Option<String>,
}

#[derive(Debug)]
struct GpmdpState {
    track: Option<GpmdpTrack>,
    is_playing: bool,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ClientId {
    Text(String),
    Art,
}

impl ClientId {
    pub fn to_owned(&self) -> ClientId {
        match *self {
            ClientId::Text(ref text) => ClientId::Text(text.to_string()),
            ClientId::Art => ClientId::Art,
        }
    }
}

type Handler = Box<Fn(&GpmdpState, &Handle) -> Box<Future<Item = (), Error = ()>> + Send>;

struct ClientState {
    current_state: GpmdpState,
    handlers: BTreeMap<ClientId, Handler>,
}

struct Client {
    id: ClientId,
    client: Arc<Mutex<ClientState>>,
    listener: Arc<ListenerHandle>,
}

impl Drop for Client {
    fn drop(&mut self) {
        self.client.lock().unwrap().handlers.remove(&self.id);
    }
}

impl Client {
    pub fn launch(id: &ClientId) -> Result<Self, String> {
        let (startup_send, startup_receive) = oneshot::channel::<Result<Remote, io::Error>>();
        let (shutdown_send, shutdown_receive) = oneshot::channel::<()>();
        let address = Url::parse("ws://127.0.0.1:5672").unwrap();
        let client = Arc::new(Mutex::new(ClientState {
            current_state: GpmdpState {
                is_playing: false,
                track: None,
            },
            handlers: BTreeMap::new(),
        }));
        let core_client = client.clone();
        let thread = thread::spawn(move || match Core::new() {
            Ok(mut core) => {
                if let Err(error) = startup_send.send(Ok(core.remote())) {
                    error!("Failed to announce startup: {:?}", error);
                    return;
                }
                let handle = core.handle();
                match core.run(Future::select2(
                    read_events(address, core_client, &handle),
                    shutdown_receive,
                )) {
                    Ok(future::Either::A(_)) => {
                        error!("client exited");
                    }
                    Ok(future::Either::B(_)) | Err(future::Either::B((_, _))) => {
                        info!("shutting down");
                    }
                    Err(future::Either::A(error)) => {
                        error!("client failed: {:?}", error);
                    }
                };
            }
            Err(err) => {
                if let Err(error) = startup_send.send(Err(err)) {
                    error!("Failed to announce startup error: {:?}", error);
                }
            }
        });
        match startup_receive.wait() {
            Ok(Ok(remote)) => Ok(Self {
                id: id.to_owned(),
                client,
                listener: Arc::new(ListenerHandle::new(thread, shutdown_send, remote)),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(format!("Failed to start core: {:?}", error))
            }
            Err(error) => {
                let _ = thread.join();
                Err(format!("Failed to start core: {:?}", error))
            }
        }
    }
}

struct ClientAccess {
    client: Mutex<(Weak<Mutex<ClientState>>, Weak<ListenerHandle>)>,
}

impl ClientAccess {
    pub fn client<F, R>(&self, id: &ClientId, action: F) -> Result<Client, String>
    where
        F: Fn(&GpmdpState, &Handle) -> R + Send + 'static,
        R: IntoFuture<Item = (), Error = ()>,
        R::Future: 'static,
    {
        let mut guard = self.client.lock().unwrap();
        let result = match (guard.0.upgrade(), guard.1.upgrade()) {
            (Some(client), Some(listener)) => Ok(Client {
                id: id.to_owned(),
                client,
                listener,
            }),
            _ => {
                let handle = Client::launch(id);
                if let Ok(ref handle) = handle {
                    *guard = (
                        Arc::downgrade(&handle.client),
                        Arc::downgrade(&handle.listener),
                    )
                }
                handle
            }
        };
        if let Ok(ref handle) = result {
            {
                let mut guard = handle.client.lock().unwrap();
                guard.handlers.insert(
                    id.to_owned(),
                    Box::new(move |s, h| Box::new(action(s, h).into_future())),
                );
                info!(
                    "added handler {:?}. there are now {} handlers.",
                    id,
                    guard.handlers.len()
                );
            }
            let spawn_client = handle.client.clone();
            let target = id.to_owned();
            handle.listener.remote.spawn(move |handle| {
                let guard = spawn_client.lock().unwrap();
                if let Some(handler) = guard.handlers.get(&target) {
                    handler(&guard.current_state, handle)
                } else {
                    Box::new(future::ok(()))
                }
            });
        }
        result
    }
}

struct GpmdpModule {}

impl obs::Module<GpmdpModule> for GpmdpModule {
    fn load() -> Option<Self> {
        let client_access = Arc::new(ClientAccess {
            client: Mutex::new((Weak::default(), Weak::default())),
        });
        obs::register_source(
            "gpmdp-album-art",
            &obs_module_text("GPMDP Album Art"),
            AlbumArtSourceDefinition::new(&client_access),
        );
        obs::register_source(
            "gpmdp-now-playing",
            &obs_module_text("GPMDP Now Playing"),
            NowPlayingSourceDefinition::new(&client_access),
        );
        Some(Self {})
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackPayload {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    album_art: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "channel", content = "payload")]
enum Message {
    PlayState(bool),
    Track(TrackPayload),
}

struct ListenerHandle {
    _thread: JoinHandle<()>,
    _shutdown: oneshot::Sender<()>,
    remote: Remote,
}

impl ListenerHandle {
    pub fn new(thread: JoinHandle<()>, shutdown: oneshot::Sender<()>, remote: Remote) -> Self {
        ListenerHandle {
            _thread: thread,
            _shutdown: shutdown,
            remote,
        }
    }
}

#[derive(Debug)]
enum ConnectionError {
    WebSocketError(WebSocketError),
    TimerError(io::Error),
}

fn read_events(
    address: Url,
    client_state: Arc<Mutex<ClientState>>,
    handle: &Handle,
) -> Box<Future<Item = (), Error = ConnectionError>> {
    let retry_delay = Duration::new(1, 0);
    let retry_handle = handle.clone();
    let websocket_handle = handle.clone();
    let update_handle = handle.clone();
    Box::new(
        ClientBuilder::from_url(&address)
            .async_connect_insecure(&websocket_handle)
            .map_err(ConnectionError::WebSocketError)
            .into_stream()
            .chain(
                stream::repeat(())
                .and_then(move |_| {
                    Timeout::new(retry_delay, &retry_handle)
                        .map_err(ConnectionError::TimerError)
                })
                .fuse() // make timer errors fatal
                .and_then(move |_| {
                    ClientBuilder::from_url(&address)
                        .async_connect_insecure(&websocket_handle)
                        .map_err(ConnectionError::WebSocketError)
                }),
            )
            .and_then(|(duplex, _)| {
                info!("connected");
                let (_, stream) = duplex.split();
                future::ok(
                    stream
                        .map_err(ConnectionError::WebSocketError)
                        .filter_map(|message| {
                            match message {
                                OwnedMessage::Text(ref text) => {
                                    match serde_json::from_str::<Message>(text) {
                                        Ok(message) => Some(message),
                                        Err(error) => {
                                            // this will log often because we only handle track messages
                                            debug!(
                                                "Failed to parse message {:?}: {:?}",
                                                message, error
                                            );
                                            None
                                        }
                                    }
                                }
                                _ => None,
                            }
                        }),
                )
            })
            .flatten()
            .then(
                move |message| -> Box<Future<Item = (), Error = ConnectionError>> {
                    match message {
                        Ok(Message::PlayState(playing)) => {
                            info!("got play state data: {:?}", playing);
                            let mut guard = client_state.lock().unwrap();
                            guard.current_state.is_playing = playing;
                            Box::new(
                                stream::futures_unordered(guard.handlers.values().map(|h| {
                                    h(&guard.current_state, &update_handle)
                                        .or_else(|_| future::ok(()))
                                })).for_each(|_| future::ok(())),
                            )
                                as Box<Future<Item = (), Error = ConnectionError>>
                        }
                        Ok(Message::Track(track)) => {
                            info!("got track data: {:?}", track);
                            let mut guard = client_state.lock().unwrap();
                            guard.current_state.track = Some(GpmdpTrack {
                                artist: track.artist,
                                album: track.album,
                                title: track.title,
                                album_art: track.album_art,
                            });
                            Box::new(
                                stream::futures_unordered(guard.handlers.values().map(|h| {
                                    h(&guard.current_state, &update_handle)
                                        .or_else(|_| future::ok(()))
                                })).for_each(|_| future::ok(())),
                            )
                                as Box<Future<Item = (), Error = ConnectionError>>
                        }
                        Err(ConnectionError::WebSocketError(e)) => {
                            debug!("got error: {:?}", e);
                            let mut guard = client_state.lock().unwrap();
                            guard.current_state.track = None;
                            guard.current_state.is_playing = false;
                            Box::new(
                                stream::futures_unordered(guard.handlers.values().map(|h| {
                                    h(&guard.current_state, &update_handle)
                                        .or_else(|_| future::ok(()))
                                })).for_each(|_| future::ok(())),
                            )
                                as Box<Future<Item = (), Error = ConnectionError>>
                        }
                        Err(e) => Box::new(future::err(e))
                            as Box<Future<Item = (), Error = ConnectionError>>,
                    }
                },
            )
            .filter(|_| false)
            .into_future()
            .then(|result| match result {
                Ok(_) => Ok(()),
                Err((err, _)) => Err(err),
            }),
    ) as Box<Future<Item = (), Error = ConnectionError>>
}
