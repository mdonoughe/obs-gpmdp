extern crate futures;
#[macro_use]
extern crate lazy_static;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio_core;
extern crate websocket;

#[macro_use]
pub mod obs;

use futures::future::{self, Either, Future};
use futures::stream::{self, Stream};
use futures::sync::oneshot;
use std::io;
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::os::raw::c_char;
use tokio_core::reactor::{Core, Timeout};
use websocket::{ClientBuilder, OwnedMessage};
use websocket::url::Url;

const NAME: *const c_char = b"obs-gpmdp\0" as *const u8 as *const c_char;
const AUTHOR: *const c_char =
    b"Matthew Donoughe <mdonoughe@gmail.com>\0" as *const u8 as *const c_char;
const DESCRIPTION: *const c_char =
    b"Display information from GPMDP.\0" as *const u8 as *const c_char;

const TARGET_TITLE: *const c_char = b"GPMDP_Title\0" as *const u8 as *const c_char;
const TARGET_ARTIST: *const c_char = b"GPMDP_Artist\0" as *const u8 as *const c_char;
const TARGET_ALBUM: *const c_char = b"GPMDP_Album\0" as *const u8 as *const c_char;
const TARGET_ARTIST_ALBUM: *const c_char = b"GPMDP_Artist_Album\0" as *const u8 as *const c_char;

const KEY_TEXT: *const c_char = b"text\0" as *const u8 as *const c_char;

struct ListenerHandle {
    thread: JoinHandle<()>,
    shutdown: oneshot::Sender<()>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackPayload {
    title: String,
    artist: String,
    album: String,
    album_art: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "channel", content = "payload")]
enum Message {
    Track(TrackPayload),
}

impl ListenerHandle {
    pub fn new(thread: JoinHandle<()>, shutdown: oneshot::Sender<()>) -> Self {
        return ListenerHandle {
            thread: thread,
            shutdown: shutdown,
        };
    }

    pub fn stop(self) -> () {
        let _ = self.shutdown.send(());
        let _ = self.thread.join();
    }
}

lazy_static! {
    static ref LISTENER: Mutex<Option<ListenerHandle>> = { Mutex::new(None) };
}

fn set_text(target: *const c_char, text: &str) {
    if let Some(mut source) = obs::get_source_by_name(target) {
        match source.get_id().as_ref() {
            "text_gdiplus" | "text_ft2_source" => {
                debug!("text should be changed to {:?}", text);
                let mut data = obs::Data::new();
                data.set(KEY_TEXT, text);
                source.update(&data);
            }
            id => {
                debug!("cannot set text on source with id {}", id);
            }
        }
    }
}

fn update_obs(track: &TrackPayload) {
    set_text(TARGET_TITLE, &track.title);
    set_text(TARGET_ARTIST, &track.artist);
    set_text(TARGET_ALBUM, &track.album);
    set_text(
        TARGET_ARTIST_ALBUM,
        &format!("{} - {}", &track.artist, &track.album),
    );
}

#[no_mangle]
pub unsafe extern "C" fn obs_module_load() -> bool {
    let (send, receive) = oneshot::channel::<Result<oneshot::Sender<()>, io::Error>>();
    let thread = thread::spawn(move || match Core::new() {
        Ok(mut core) => {
            let (shutdown_send, shutdown_receive) = oneshot::channel::<()>();
            if let Err(error) = send.send(Ok(shutdown_send)) {
                error!("Failed to return shutdown handle: {:?}", error);
                return;
            }
            let address = Url::parse("ws://127.0.0.1:5672").unwrap();
            let handle = core.handle();
            let runner = stream::repeat::<_, websocket::WebSocketError>(())
                .and_then(|_| {
                    future::ok(
                        ClientBuilder::from_url(&address)
                            .async_connect_insecure(&handle)
                            .into_stream()
                            .chain(
                                future::lazy(|| Timeout::new(Duration::new(1, 0), &handle))
                                    .into_stream()
                                    .then(|_| Ok(()))
                                    .filter_map(|_| None),
                            ),
                    )
                })
                .flatten()
                .and_then(|(duplex, _)| {
                    info!("connected");
                    let (_, stream) = duplex.split();
                    future::ok(stream.filter_map(|message| {
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
                    }))
                })
                .flatten()
                .then(|result| future::ok::<_, ()>(result))
                .for_each(|message| {
                    match message {
                        Ok(Message::Track(track)) => {
                            info!("got data: {:?}", track);
                            update_obs(&track);
                        }
                        Err(e) => {
                            debug!("got error: {:?}", e);
                        }
                    }
                    future::ok(())
                });
            match core.run(Future::select2(runner, shutdown_receive)) {
                Ok(Either::A(_)) => {
                    error!("disconnected?");
                }
                Ok(Either::B(_)) => {
                    info!("shutting down");
                }
                Err(Either::A(error)) => {
                    error!("reactor failed: {:?}", error);
                }
                Err(Either::B(_)) => {
                    error!("shutdown due to error");
                }
            }
        }
        Err(error) => {
            let _ = send.send(Err(error));
        }
    });
    match receive.wait() {
        Ok(Ok(shutdown)) => {
            let mut guard = LISTENER.lock().unwrap();
            *guard = Some(ListenerHandle::new(thread, shutdown));
            true
        }
        Ok(Err(error)) => {
            error!("Failed to start core: {:?}", error);
            let _ = thread.join();
            false
        }
        Err(error) => {
            error!("Failed to start core: {:?}", error);
            if let Err(thread_error) = thread.join() {
                error!("Thread error: {:?}", thread_error);
            }
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn obs_module_unload() -> () {
    let mut guard = LISTENER.lock().unwrap();
    match guard.take() {
        Some(listener) => {
            listener.stop();
        }
        None => {}
    };
}

#[no_mangle]
pub unsafe extern "C" fn obs_module_name() -> *const c_char {
    NAME
}

#[no_mangle]
pub unsafe extern "C" fn obs_module_author() -> *const c_char {
    AUTHOR
}

#[no_mangle]
pub unsafe extern "C" fn obs_module_description() -> *const c_char {
    DESCRIPTION
}
