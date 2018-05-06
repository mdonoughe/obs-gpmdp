use std::ffi::CString;
use std::marker::PhantomData;
use libobs;

pub struct Properties(Option<*mut libobs::obs_properties_t>);

impl Properties {
    pub fn new() -> Self {
        unsafe { Properties(Some(libobs::obs_properties_create())) }
    }

    pub(super) unsafe fn from_raw(raw: *mut libobs::obs_properties_t) -> Self {
        Properties(Some(raw))
    }

    pub(super) unsafe fn into_ptr(mut self) -> *mut libobs::obs_properties_t {
        self.0.take().unwrap()
    }

    pub fn get_property<'a>(&self, name: &str) -> Option<Property<'a>> {
        unsafe {
            let name = CString::new(name).unwrap();
            let ptr = libobs::obs_properties_get(self.0.unwrap(), name.as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(Property {
                    property: ptr,
                    marker: PhantomData,
                })
            }
        }
    }
}

impl Drop for Properties {
    fn drop(&mut self) {
        unsafe {
            if let Some(properties) = self.0.take() {
                libobs::obs_properties_destroy(properties);
            }
        }
    }
}

pub struct Property<'a> {
    property: *mut libobs::obs_property_t,
    marker: PhantomData<&'a ()>,
}

impl<'a> Property<'a> {
    pub fn set_visible(&mut self, visibility: bool) {
        unsafe {
            libobs::obs_property_set_visible(self.property, visibility);
        }
    }
    pub fn set_description(&mut self, description: &str) {
        unsafe {
            let description = CString::new(description).unwrap();
            libobs::obs_property_set_description(self.property, description.as_ptr())
        }
    }
}
