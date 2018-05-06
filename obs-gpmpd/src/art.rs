use {Client, ClientAccess, ClientId};
use futures::prelude::*;
use futures::future;
use hyper::{self, mime, Method, Request, Response, StatusCode, Uri};
use hyper::header::{q, Accept, ContentLength, ContentType, QualityItem};
use hyper_tls::HttpsConnector;
use image::{self, ImageFormat, RgbaImage};
use obs::{self, Data, ObsSource, Texture, VideoSource, VideoSourceDefinition};
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, Mutex, Weak};
use tokio_core::reactor::Handle;

// 4MB: enough for a 1024x1024 ARGB raw bitmap.
// for comparison, 3 minutes at 320kbps is about 7MB.
const MAXIMUM_LENGTH: u64 = 4 * 1024 * 1024;

fn decode_image(
    response: Response,
    address: &Rc<String>,
) -> Box<Future<Item = RgbaImage, Error = String>> {
    let length_okay = match response.headers().get() {
        Some(&ContentLength(length)) if length > MAXIMUM_LENGTH => Err(format!(
            "rejecting album art from {:?} because it is too large ({}MB)",
            address,
            length / (1024 * 1024)
        )),
        _ => Ok(()),
    };
    let format = match response
        .headers()
        .get()
        .and_then(|t: &ContentType| Some((t.type_(), t.subtype())))
    {
        Some((mime::IMAGE, subtype)) if subtype == "webp" => Ok(ImageFormat::WEBP),
        Some((mime::IMAGE, mime::JPEG)) => Ok(ImageFormat::JPEG),
        Some((mime::IMAGE, mime::PNG)) => Ok(ImageFormat::PNG),
        other => Err(format!(
            "rejecting album art from {:?} because {:?} is not a supported image type",
            address, other
        )),
    };
    let address = address.clone();
    Box::new(
        length_okay
            .and(format)
            .into_future()
            .and_then(move |format| {
                //TODO: download, but reject as soon as it becomes too large
                let decode_error_address = address.clone();
                response
                    .body()
                    .concat2()
                    .map_err(move |err| {
                        format!("art download from {:?} failed: {:?}", address, err)
                    })
                    .and_then(move |body| {
                        image::load_from_memory_with_format(&body, format)
                            .and_then(|image| Ok(image.to_rgba()))
                            .map_err(|err| {
                                format!(
                                    "could not decode album art from {:?} as {:?}: {:?}",
                                    decode_error_address, format, err
                                )
                            })
                    })
            }),
    )
}

pub fn load(address: &str, handle: &Handle) -> Box<Future<Item = RgbaImage, Error = String>> {
    let address = Rc::new(address.to_string());
    let parse_error_address = address.clone();
    let client = hyper::Client::configure()
        .connector(HttpsConnector::new(4, handle).unwrap())
        .build(handle);
    Box::new(
        Uri::from_str(&address)
            .map_err(move |err| {
                format!(
                    "could not parse album art URI {:?}: {:?}",
                    parse_error_address, err
                )
            })
            .into_future()
            .and_then(move |uri| {
                let mut request = Request::new(Method::Get, uri);
                request.headers_mut().set(Accept(vec![
                    QualityItem::new("image/webp".parse().unwrap(), q(1000)),
                    QualityItem::new(mime::IMAGE_JPEG, q(900)),
                    QualityItem::new(mime::IMAGE_PNG, q(800)),
                ]));
                let request_error_address = address.clone();
                client
                    .request(request)
                    .map_err(move |err| {
                        format!(
                            "could not load album art from {:?}: {:?}",
                            request_error_address, err
                        )
                    })
                    .and_then(move |response| match response.status() {
                        StatusCode::Ok => decode_image(response, &address),
                        status => Box::new(future::err(format!(
                            "got unexpected status code {:?} for {:?}",
                            status, address
                        ))),
                    })
            }),
    )
}

pub(super) struct AlbumArtSourceDefinition {
    client_access: Arc<ClientAccess>,
    client: Mutex<Weak<ArtClient>>,
}

// used to deal with sending data to a thread where we do not own the stack.
// Mutex would work without having to hack anything, but then the mutex needs
// to be obtained on the render thread three times per album art source per frame.
struct UnsafeSync<T>(T);

unsafe impl<T> Sync for UnsafeSync<T> {}

struct ArtData {
    is_playing: bool,
    texture: Option<Texture>,
}

struct ArtClient {
    _client: Client,
    // only access from the render thread!
    data: Arc<UnsafeSync<RefCell<ArtData>>>,
}

impl AlbumArtSourceDefinition {
    pub fn new(client_access: &Arc<ClientAccess>) -> Self {
        AlbumArtSourceDefinition {
            client_access: client_access.clone(),
            client: Mutex::new(Weak::default()),
        }
    }
}

impl VideoSourceDefinition for AlbumArtSourceDefinition {
    type Source = AlbumArtSource;
    fn create(&self, _settings: &Data, _source: &mut ObsSource) -> Self::Source {
        let mut guard = self.client.lock().unwrap();
        let art_client = match guard.upgrade() {
            Some(art_client) => Some(art_client),
            None => {
                let data = Arc::new(UnsafeSync(RefCell::new(ArtData {
                    is_playing: false,
                    texture: None,
                })));
                let art_address: RefCell<Option<String>> = RefCell::new(None);
                let update_data = data.clone();
                let client = self.client_access.client(&ClientId::Art, move |s, handle| {
                    let is_playing = s.is_playing;
                    let mut art_address = art_address.borrow_mut();
                    let update_data = update_data.clone();
                    let address = s.track
                        .as_ref()
                        .and_then(|s| s.album_art.as_ref())
                        .map(|s| s.as_str());
                    let result = match (&*art_address, address) {
                        (_, None) => Box::new(future::ok(Some(None))),
                        (&Some(ref a), Some(b)) if a == b => Box::new(future::ok(None)),
                        (_, Some(address)) => {
                            let err_address = address.to_string();
                            Box::new(
                                load(address, handle)
                                    .and_then(move |image| unsafe {
                                        let _graphics = obs::enter_graphics();
                                        future::ok(Some(Some(Texture::new(&image))))
                                    })
                                    .or_else(move |err| {
                                        println!(
                                            "failed to load art from {}: {:?}",
                                            err_address, err
                                        );
                                        future::ok(Some(None))
                                    }),
                            )
                                as Box<Future<Item = Option<Option<Texture>>, Error = ()>>
                        }
                    }.and_then(move |image| {
                        let update_data = update_data.clone();
                        Box::new(obs::execute_main_render_callback(move |_, _| {
                            let data = &mut *update_data.0.borrow_mut();
                            data.is_playing = is_playing;
                            if let Some(image) = image {
                                data.texture = image;
                            }
                            Ok(())
                        })) as Box<Future<Item = (), Error = ()>>
                    });
                    *art_address = address.map(|a| a.to_string());
                    result
                });
                match client {
                    Ok(client) => {
                        let art_client = Arc::new(ArtClient {
                            _client: client,
                            data,
                        });
                        *guard = Arc::downgrade(&art_client);
                        Some(art_client)
                    }
                    Err(e) => {
                        error!("failed to launch client to load art: {:?}", e);
                        None
                    }
                }
            }
        };
        AlbumArtSource { client: art_client }
    }
}

pub struct AlbumArtSource {
    client: Option<Arc<ArtClient>>,
}

impl VideoSource for AlbumArtSource {
    fn get_width(&self) -> u32 {
        // obs doesn't like 0x0 sources
        self.client
            .as_ref()
            .and_then(|client| client.data.0.borrow().texture.as_ref().map(|t| t.width()))
            .unwrap_or(1)
    }
    fn get_height(&self) -> u32 {
        // obs doesn't like 0x0 sources
        self.client
            .as_ref()
            .and_then(|client| client.data.0.borrow().texture.as_ref().map(|t| t.height()))
            .unwrap_or(1)
    }
    fn video_render(&mut self) {
        if let Some(ref client) = self.client {
            let data = client.data.0.borrow();
            if data.is_playing {
                if let Some(ref texture) = data.texture {
                    texture.draw();
                }
            }
        }
    }
}
