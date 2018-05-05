use std::ffi::{CStr, CString};
use super::libobs;

pub struct Data(*mut libobs::obs_data_t);

impl Data {
    pub fn new() -> Self {
        unsafe { Data(libobs::obs_data_create()) }
    }

    pub(super) unsafe fn from_raw(raw: *mut libobs::obs_data_t) -> Self {
        Data(raw)
    }

    pub(super) unsafe fn as_raw(&self) -> *mut libobs::obs_data_t {
        self.0
    }

    pub fn set_string(&mut self, key: &str, value: &str) {
        unsafe {
            libobs::obs_data_set_string(
                self.0,
                CString::new(key).unwrap().as_ptr(),
                CString::new(value).unwrap().as_ptr(),
            );
        }
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        unsafe {
            let ptr = libobs::obs_data_get_string(self.0, CString::new(key).unwrap().as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
            }
        }
    }

    pub fn set_default_string(&mut self, key: &str, value: &str) {
        unsafe {
            libobs::obs_data_set_default_string(
                self.0,
                CString::new(key).unwrap().as_ptr(),
                CString::new(value).unwrap().as_ptr(),
            );
        }
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        unsafe {
            libobs::obs_data_set_bool(self.0, CString::new(key).unwrap().as_ptr(), value);
        }
    }

    pub fn get_bool(&self, key: &str) -> bool {
        unsafe { libobs::obs_data_get_bool(self.0, CString::new(key).unwrap().as_ptr()) }
    }

    pub fn apply(&mut self, other: &Self) {
        unsafe {
            libobs::obs_data_apply(self.0, other.0);
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
