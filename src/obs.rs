use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};

#[link(name = "obs")]
extern "C" {
    fn blog(log_level: i32, format: *const c_char, ...);
    fn obs_get_source_by_name(name: *const c_char) -> *const c_void;
    fn obs_source_release(source: *const c_void);
    fn obs_source_get_id(source: *const c_void) -> *const c_char;
    fn obs_source_update(source: *const c_void, data: *const c_void);
    fn obs_data_create() -> *const c_void;
    fn obs_data_release(data: *const c_void);
    fn obs_data_set_string(data: *const c_void, key: *const c_char, value: *const c_char);
}

pub const DUMMY_LOG_TEMPLATE: *const c_char = b"gpmdp: %s\0" as *const u8 as *const c_char;

pub fn obs_log(level: i32, text: String) {
    unsafe {
        blog(
            level,
            DUMMY_LOG_TEMPLATE,
            CString::new(text).unwrap().into_raw(),
        );
    }
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => ($crate::obs::obs_log(400, $crate::std::fmt::format(format_args!($($arg)*))));
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => ($crate::obs::obs_log(300, $crate::std::fmt::format(format_args!($($arg)*))));
}

#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => ($crate::obs::obs_log(200, $crate::std::fmt::format(format_args!($($arg)*))));
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => ($crate::obs::obs_log(100, $crate::std::fmt::format(format_args!($($arg)*))));
}

pub struct Data(*const c_void);

impl Data {
    pub fn new() -> Self {
        unsafe { Data(obs_data_create()) }
    }

    pub fn set(&mut self, key: *const c_char, value: &str) {
        unsafe {
            obs_data_set_string(self.0, key, CString::new(value).unwrap().as_ptr());
        }
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        unsafe {
            obs_data_release(self.0);
        }
    }
}

pub struct Source(*const c_void);

impl Source {
    pub fn get_id(&self) -> String {
        unsafe {
            CStr::from_ptr(obs_source_get_id(self.0))
                .to_string_lossy()
                .into_owned()
        }
    }

    pub fn update(&mut self, data: &Data) {
        unsafe {
            obs_source_update(self.0, data.0);
        }
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        unsafe {
            obs_source_release(self.0);
        }
    }
}

pub fn get_source_by_name(name: *const c_char) -> Option<Source> {
    unsafe {
        match obs_get_source_by_name(name) {
            nil if nil.is_null() => None,
            ptr => Some(Source(ptr)),
        }
    }
}

const LIBOBS_API_MAJOR_VER: u8 = 21;
const LIBOBS_API_MINOR_VER: u8 = 0;
const LIBOBS_API_PATCH_VER: u16 = 2;
const LIBOBS_API_VER: u32 = ((LIBOBS_API_MAJOR_VER as u32) << 24)
    | ((LIBOBS_API_MINOR_VER as u32) << 16)
    | LIBOBS_API_PATCH_VER as u32;

static mut OBS_MODULE_POINTER: Option<*const i32> = None;

#[no_mangle]
pub unsafe extern "C" fn obs_module_set_pointer(module: *const i32) -> () {
    OBS_MODULE_POINTER = Some(module);
}

#[no_mangle]
pub unsafe extern "C" fn obs_module_ver() -> u32 {
    LIBOBS_API_VER
}
