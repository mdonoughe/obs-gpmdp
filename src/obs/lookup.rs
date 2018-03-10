use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;
use super::libobs;

pub struct Lookup(*mut libobs::lookup_t);

impl Lookup {
    pub(super) fn from_raw(lookup: *mut libobs::lookup_t) -> Self {
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
