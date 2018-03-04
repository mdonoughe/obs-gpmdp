use image::RgbaImage;
use libobs;
use std::ffi::{CStr, CString};
use std::{mem, ptr};
use std::os::raw::{c_char, c_void};

pub const DUMMY_LOG_TEMPLATE: *const c_char = b"[gpmdp] %s\0" as *const u8 as *const c_char;

pub fn blog(level: i32, text: String) {
    unsafe {
        libobs::blog(
            level,
            DUMMY_LOG_TEMPLATE,
            CString::new(text).unwrap().as_ptr(),
        );
    }
}

pub struct Data(*mut libobs::obs_data);

impl Data {
    pub fn new() -> Self {
        unsafe { Data(libobs::obs_data_create()) }
    }

    pub fn set(&mut self, key: *const c_char, value: &str) {
        unsafe {
            libobs::obs_data_set_string(self.0, key, CString::new(value).unwrap().as_ptr());
        }
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        unsafe {
            libobs::obs_data_release(self.0);
        }
    }
}

pub struct ObsSource(*mut libobs::obs_source);

impl ObsSource {
    pub fn get_id(&self) -> String {
        unsafe {
            CStr::from_ptr(libobs::obs_source_get_id(self.0))
                .to_string_lossy()
                .into_owned()
        }
    }

    pub fn update(&mut self, data: &Data) {
        unsafe {
            libobs::obs_source_update(self.0, data.0);
        }
    }
}

impl Drop for ObsSource {
    fn drop(&mut self) {
        unsafe {
            libobs::obs_source_release(self.0);
        }
    }
}

impl Clone for ObsSource {
    fn clone(&self) -> Self {
        unsafe {
            libobs::obs_source_addref(self.0);
        }
        ObsSource(self.0)
    }
}

pub fn get_source_by_name(name: *const c_char) -> Option<ObsSource> {
    unsafe {
        match libobs::obs_get_source_by_name(name) {
            nil if nil.is_null() => None,
            ptr => Some(ObsSource(ptr)),
        }
    }
}

pub trait VideoSource {
    fn update(&mut self, _settings: &Data) {}
    fn get_width(&self) -> u32;
    fn get_height(&self) -> u32;
    fn video_tick(&mut self, _seconds: f32) {}
    fn video_render(&mut self) {}
}

struct SourceDefinition {
    pub name: CString,
    pub create: Box<Fn(&Data, &ObsSource) -> Box<VideoSource>>,
}

unsafe extern "C" fn source_get_name(data: *mut c_void) -> *const c_char {
    let data = &*(data as *mut SourceDefinition);
    data.name.as_ptr()
}

unsafe extern "C" fn source_free_type_data(data: *mut c_void) {
    mem::drop(Box::from_raw(data as *mut SourceDefinition))
}

unsafe extern "C" fn source_create(
    settings: *mut libobs::obs_data,
    source: *mut libobs::obs_source,
) -> *mut c_void {
    let data = &*((&*source).info.type_data as *mut SourceDefinition);

    // increment because our wrappers are going to decrement on drop
    libobs::obs_data_addref(settings);
    libobs::obs_source_addref(source);
    Box::into_raw(Box::new((data.create)(&Data(settings), &ObsSource(source)))) as *mut c_void
}

unsafe extern "C" fn source_destroy(source: *mut c_void) {
    mem::drop(Box::from_raw(source as *mut Box<VideoSource>))
}

unsafe extern "C" fn source_update(source: *mut c_void, settings: *mut libobs::obs_data) {
    let source = &mut *(source as *mut Box<VideoSource>);
    // increment because our wrappers are going to decrement on drop
    libobs::obs_data_addref(settings);
    source.update(&Data(settings));
}

unsafe extern "C" fn source_get_width(source: *mut c_void) -> u32 {
    let source = &mut *(source as *mut Box<VideoSource>);
    source.get_width()
}

unsafe extern "C" fn source_get_height(source: *mut c_void) -> u32 {
    let source = &mut *(source as *mut Box<VideoSource>);
    source.get_height()
}

unsafe extern "C" fn source_video_tick(source: *mut c_void, seconds: f32) {
    let source = &mut *(source as *mut Box<VideoSource>);
    source.video_tick(seconds);
}

unsafe extern "C" fn source_video_render(source: *mut c_void, _effect: *mut libobs::gs_effect) {
    let source = &mut *(source as *mut Box<VideoSource>);
    source.video_render();
}

pub fn register_source<S: 'static, C: 'static>(id: *const c_char, name: &str, create: C)
where
    S: VideoSource,
    C: Fn(&Data, &ObsSource) -> S,
{
    unsafe {
        let mut si: libobs::obs_source_info = mem::zeroed();
        si.id = id;
        si.type_data = Box::into_raw(Box::new(SourceDefinition {
            name: CString::new(name).unwrap(),
            create: Box::new(move |settings, source| Box::new(create(settings, source))),
        })) as *mut c_void;
        si.free_type_data = Some(source_free_type_data);
        si.type_ = libobs::obs_source_type::OBS_SOURCE_TYPE_INPUT;
        si.output_flags = libobs::OBS_SOURCE_VIDEO;
        si.get_name = Some(source_get_name);
        si.create = Some(source_create);
        si.destroy = Some(source_destroy);
        si.update = Some(source_update);
        si.get_width = Some(source_get_width);
        si.get_height = Some(source_get_height);
        si.video_tick = Some(source_video_tick);
        si.video_render = Some(source_video_render);
        libobs::obs_register_source_s(
            &si as *const libobs::obs_source_info,
            mem::size_of::<libobs::obs_source_info>(),
        );
    }
}

pub struct Texture {
    texture: *mut libobs::gs_texture_t,
    width: u32,
    height: u32,
}
impl Texture {
    pub fn new(image: &RgbaImage) -> Self {
        let (width, height) = image.dimensions();
        unsafe {
            let mut scans = vec![ptr::null() as *const u8; height as usize];
            for i in 0..height {
                scans[i as usize] = mem::transmute(image.get_pixel(0, i));
            }

            let texture = libobs::gs_texture_create(
                width,
                height,
                libobs::gs_color_format::GS_RGBA,
                1,
                scans.as_ptr() as *mut *const u8,
                0,
            );
            Texture {
                texture: texture,
                width: width,
                height: height,
            }
        }
    }
    pub fn draw(&self) {
        unsafe {
            libobs::obs_source_draw(self.texture, 0, 0, 0, 0, false);
        }
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
}
impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            libobs::gs_texture_destroy(self.texture);
        }
    }
}

pub trait Module {
    fn load() -> Option<Box<Self>>;
}

pub struct Lookup(*mut libobs::lookup_t);

impl Lookup {
    pub fn new(lookup: *mut libobs::lookup_t) -> Self {
        Lookup(lookup)
    }

    pub fn getstr(&self, val: &str) -> Option<String> {
        unsafe {
            let mut ptr: *const c_char = ptr::null();
            if libobs::text_lookup_getstr(
                self.0,
                val.as_bytes().as_ptr() as *const c_char,
                &mut ptr,
            ) {
                Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
            } else {
                None
            }
        }
    }
}

impl Drop for Lookup {
    fn drop(&mut self) {
        unsafe {
            libobs::text_lookup_destroy(self.0);
        }
    }
}

unsafe impl Send for Lookup {}
unsafe impl Sync for Lookup {}
