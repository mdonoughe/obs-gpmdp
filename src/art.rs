use futures::{future, Future, Stream};
use hyper::{mime, Client, Method, Request, StatusCode, Uri};
use hyper::header::{q, Accept, ContentLength, ContentType, QualityItem};
use hyper_tls::HttpsConnector;
use image::{self, ImageFormat, RgbaImage};
use obs::{ObsSource, Texture, VideoSource};
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio_core::reactor::Handle;

// 4MB: enough for a 1024x1024 ARGB raw bitmap.
// for comparison, 3 minutes at 320kbps is about 7MB.
const MAXIMUM_LENGTH: u64 = 4 * 1024 * 1024;

pub fn load(
    address: &str,
    handle: &Handle,
) -> Box<Future<Item = Option<RgbaImage>, Error = String>> {
    let address = Rc::new(address.to_string());
    let parse_error_address = address.clone();
    let client = Client::configure()
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
            .and_then(move |uri| {
                let mut request = Request::new(Method::Get, uri);
                request.headers_mut().set(Accept(vec![
                    QualityItem::new("image/webp".parse().unwrap(), q(1000)),
                    QualityItem::new(mime::IMAGE_JPEG, q(900)),
                    QualityItem::new(mime::IMAGE_PNG, q(800)),
                ]));
                let request_error_address = address.clone();
                Ok(Box::new(
                    client
                        .request(request)
                        .map_err(move |err| {
                            format!(
                                "could not load album art from {:?}: {:?}",
                                request_error_address, err
                            )
                        })
                        .and_then(move |response| {
                            match response.status() {
                                StatusCode::Ok => {
                                    let format = match response.headers().get().and_then(|t: &ContentType| Some((t.type_(), t.subtype()))) {
                                        Some((mime::IMAGE, subtype)) if subtype == "webp" => Ok(ImageFormat::WEBP),
                                        Some((mime::IMAGE, mime::JPEG)) => Ok(ImageFormat::JPEG),
                                        Some((mime::IMAGE, mime::PNG)) => Ok(ImageFormat::PNG),
                                        other => {
                                            Err(format!("rejecting album art from {:?} because {:?} is not a supported image type", address, other))
                                        }
                                    }?;
                                    match response.headers().get() {
                                        Some(&ContentLength(length)) if length > MAXIMUM_LENGTH => {
                                            Err(format!("rejecting album art from {:?} because it is too large ({}MB)", address, length / (1024 * 1024)))
                                        },
                                        _ => {
                                            //TODO: download, but reject as soon as it becomes too large
                                            Ok(response.body()
                                                .concat2()
                                                .map_err(move |err| format!("art download from {:?} failed: {:?}", address, err))
                                                .map(move |body| (body.to_vec(), format)))
                                        }
                                    }
                                }
                                status => Err(format!(
                                    "got unexpected status code {:?} for {:?}",
                                    status, address
                                )),
                            }
                        }),
                ) as Box<Future<Item = _, Error = _>>)
            })
            .unwrap_or_else(|err| Box::new(future::err(err)))
            .and_then(|inner| inner)
            .and_then(|(body, format): (Vec<u8>, _)| {
                future::result(
                    image::load_from_memory_with_format(&body, format)
                        .and_then(|image| Ok(Some(image.to_rgba())))
                        .map_err(|err| format!("could not decode album art: {:?}", err)),
                )
            }),
    )
}

pub struct AlbumArtSource {
    new_art: Arc<Mutex<Option<RgbaImage>>>,
    texture: Arc<RefCell<Option<Texture>>>,
}
impl AlbumArtSource {
    pub fn new(
        _source: &ObsSource,
        new_art: &Arc<Mutex<Option<RgbaImage>>>,
        texture: &Arc<RefCell<Option<Texture>>>,
    ) -> Self {
        AlbumArtSource {
            new_art: new_art.clone(),
            texture: texture.clone(),
        }
    }
}
impl VideoSource for AlbumArtSource {
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
            *texture = Some(Texture::new(&image));
            debug!("activated image {}x{}", image.width(), image.height());
        }

        if let Some(ref texture) = *self.texture.borrow() {
            texture.draw();
        }
    }
}
