extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate image;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio_core;
extern crate websocket;

mod macros;
mod art;
mod libobs;
pub mod obs;

use futures::{future, stream, Future, Stream};
use futures::sync::oneshot;
use image::{Pixel, RgbaImage};
use std::io;
use std::cell::RefCell;
use std::os::raw::c_char;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tokio_core::reactor::{Core, Handle, Timeout};
use websocket::{ClientBuilder, OwnedMessage, WebSocketError};
use websocket::url::Url;

const NAME: *const c_char = b"obs-gpmdp\0" as *const u8 as *const c_char;
const AUTHOR: *const c_char =
    b"Matthew Donoughe <mdonoughe@gmail.com>\0" as *const u8 as *const c_char;
const DESCRIPTION: *const c_char =
    b"Display information from GPMDP.\0" as *const u8 as *const c_char;

const TARGET_TITLE: *const c_char = b"GPMDP Title\0" as *const u8 as *const c_char;
const TARGET_ARTIST: *const c_char = b"GPMDP Artist\0" as *const u8 as *const c_char;
const TARGET_ALBUM: *const c_char = b"GPMDP Album\0" as *const u8 as *const c_char;
const TARGET_ARTIST_ALBUM: *const c_char = b"GPMDP Artist Album\0" as *const u8 as *const c_char;

const KEY_TEXT: *const c_char = b"text\0" as *const u8 as *const c_char;

const ID_ART: *const c_char = b"gpmdp-album-art\0" as *const u8 as *const c_char;

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
    album_art: Option<String>,
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

static mut LISTENER: Option<ListenerHandle> = None;

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

fn clear_obs() {
    set_text(TARGET_TITLE, "");
    set_text(TARGET_ARTIST, "");
    set_text(TARGET_ALBUM, "");
    set_text(TARGET_ARTIST_ALBUM, "");
}

#[derive(Debug)]
enum ConnectionError {
    WebSocketError(WebSocketError),
    TimerError(io::Error),
}

fn read_events(
    address: Url,
    art: Arc<Mutex<Option<RgbaImage>>>,
    handle: &Handle,
) -> Box<Future<Item = (), Error = ConnectionError>> {
    let retry_delay = Duration::new(1, 0);
    let retry_handle = handle.clone();
    let websocket_handle = handle.clone();
    let art_handle = handle.clone();
    let art_address: RefCell<Option<String>> = RefCell::new(None);
    Box::new(
        ClientBuilder::from_url(&address)
            .async_connect_insecure(&websocket_handle)
            .map_err(|err| ConnectionError::WebSocketError(err))
            .into_stream()
            .chain(
                stream::repeat(())
                .and_then(move |_| {
                    Timeout::new(retry_delay, &retry_handle).map_err(|err| ConnectionError::TimerError(err))
                })
                .fuse() // make timer errors fatal
                .and_then(move |_| {
                    ClientBuilder::from_url(&address)
                        .async_connect_insecure(&websocket_handle)
                        .map_err(|err| ConnectionError::WebSocketError(err))
                }),
            )
            .and_then(|(duplex, _)| {
                info!("connected");
                let (_, stream) = duplex.split();
                future::ok(
                    stream
                        .map_err(|err| ConnectionError::WebSocketError(err))
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
                        Ok(Message::Track(track)) => {
                            info!("got data: {:?}", track);
                            update_obs(&track);
                            let mut art_address = art_address.borrow_mut();
                            let art = art.clone();
                            match track.album_art {
                                Some(ref url)
                                    if art_address
                                        .as_ref()
                                        .map_or(true, |current| current != url) =>
                                {
                                    *art_address = Some(url.to_string());
                                    Box::new(art::load(&url, &art_handle).then(move |result| {
                                        match result {
                                            Ok(new_art) => {
                                                *art.lock().unwrap() = new_art;
                                                info!("art loaded");
                                            }
                                            Err(err) => {
                                                error!("{}", err);
                                            }
                                        }
                                        future::ok(())
                                    }))
                                }
                                Some(_) => Box::new(future::ok(())),
                                None => {
                                    *art.lock().unwrap() = None;
                                    //TODO: clear art
                                    Box::new(future::ok(()))
                                        as Box<Future<Item = (), Error = ConnectionError>>
                                }
                            }
                        }
                        Err(ConnectionError::WebSocketError(e)) => {
                            debug!("got error: {:?}", e);
                            clear_obs();
                            Box::new(future::ok(()))
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

#[no_mangle]
pub unsafe extern "C" fn obs_module_load() -> bool {
    let art = Arc::new(Mutex::new(Some(RgbaImage::from_fn(128, 128, |x, y| {
        image::Rgba::from_channels(x as u8, y as u8, 127u8, 255u8)
    }))));
    let load_art_mutex = art.clone();
    let texture = Arc::new(RefCell::new(None));
    obs::register_source(ID_ART, "GPMDP Album Art", move |_, source| {
        AlbumArtSource::new(source, &art, &texture)
    });

    let (startup_send, startup_receive) = oneshot::channel::<Result<(), io::Error>>();
    let (shutdown_send, shutdown_receive) = oneshot::channel::<()>();
    let address = Url::parse("ws://127.0.0.1:5672").unwrap();
    let thread = thread::spawn(move || match Core::new() {
        Ok(mut core) => {
            if let Err(error) = startup_send.send(Ok(())) {
                error!("Failed to announce startup: {:?}", error);
                return;
            }
            let handle = core.handle();
            match core.run(Future::select2(
                read_events(address, load_art_mutex, &handle),
                shutdown_receive,
            )) {
                Ok(future::Either::A(_)) => {
                    error!("client exited");
                }
                Ok(future::Either::B(_)) => {
                    info!("shutting down");
                }
                Err(future::Either::A(error)) => {
                    error!("client failed: {:?}", error);
                }
                Err(future::Either::B((error, _))) => {
                    error!("shutdown listener failed: {:?}", error);
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
        Ok(Ok(())) => {
            LISTENER = Some(ListenerHandle::new(thread, shutdown_send));
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
    if let Some(listener) = LISTENER.take() {
        listener.stop();
    }
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

struct AlbumArtSource {
    new_art: Arc<Mutex<Option<RgbaImage>>>,
    texture: Arc<RefCell<Option<obs::Texture>>>,
}
impl AlbumArtSource {
    pub fn new(
        _source: &obs::ObsSource,
        new_art: &Arc<Mutex<Option<RgbaImage>>>,
        texture: &Arc<RefCell<Option<obs::Texture>>>,
    ) -> Self {
        AlbumArtSource {
            new_art: new_art.clone(),
            texture: texture.clone(),
        }
    }
}
impl obs::VideoSource for AlbumArtSource {
    fn get_width(&self) -> u32 {
        (*self.texture).borrow().as_ref().map_or(0, |t| t.width())
    }
    fn get_height(&self) -> u32 {
        (*self.texture).borrow().as_ref().map_or(0, |t| t.height())
    }
    fn video_render(&mut self) {
        let mut guard = self.new_art.lock().unwrap();
        if let Some(image) = guard.take() {
            let mut texture = (*self.texture).borrow_mut();
            *texture = Some(obs::Texture::new(&image));
            debug!("activated image {}x{}", image.width(), image.height());
        }

        if let Some(ref texture) = *self.texture.borrow() {
            texture.draw();
        }
    }
}
