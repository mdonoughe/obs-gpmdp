use std::ffi::CString;
use std::os::raw::c_char;
use libobs;

const DUMMY_LOG_TEMPLATE: *const c_char = b"[gpmdp] %s\0" as *const u8 as *const c_char;

pub fn blog(level: i32, text: String) {
    unsafe {
        let text = CString::new(text).unwrap();
        libobs::blog(level, DUMMY_LOG_TEMPLATE, text.as_ptr());
    }
}
