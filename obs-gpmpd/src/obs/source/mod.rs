mod ffi;

use libobs;
use std::ffi::{CStr, CString};
use std::{mem, ptr};
use std::os::raw::c_void;
use super::{Data, Properties};

pub struct ObsSource(*mut libobs::obs_source);

impl ObsSource {
    pub fn get_name(&self) -> String {
        unsafe {
            CStr::from_ptr(libobs::obs_source_get_name(self.0))
                .to_string_lossy()
                .into_owned()
        }
    }

    pub fn update(&self, data: &Data) {
        unsafe {
            libobs::obs_source_update(self.0, data.as_raw());
        }
    }

    pub fn get_width(&self) -> u32 {
        unsafe { libobs::obs_source_get_width(self.0) }
    }

    pub fn get_height(&self) -> u32 {
        unsafe { libobs::obs_source_get_height(self.0) }
    }

    pub fn video_render(&self) {
        unsafe {
            libobs::obs_source_video_render(self.0);
        }
    }

    pub fn get_properties(&self) -> Properties {
        unsafe { Properties::from_raw(libobs::obs_source_properties(self.0)) }
    }

    pub fn get_weak_source(&self) -> ObsWeakSource {
        unsafe { ObsWeakSource(libobs::obs_source_get_weak_source(self.0)) }
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

pub struct ObsWeakSource(*mut libobs::obs_weak_source_t);

impl ObsWeakSource {
    pub fn upgrade(&self) -> Option<ObsSource> {
        unsafe {
            let ptr = libobs::obs_weak_source_get_source(self.0);
            if ptr.is_null() {
                None
            } else {
                Some(ObsSource(ptr))
            }
        }
    }
}

impl Clone for ObsWeakSource {
    fn clone(&self) -> Self {
        unsafe {
            libobs::obs_weak_source_addref(self.0);
        }
        ObsWeakSource(self.0)
    }
}

impl Drop for ObsWeakSource {
    fn drop(&mut self) {
        unsafe {
            libobs::obs_weak_source_release(self.0);
        }
    }
}

unsafe impl Send for ObsWeakSource {}
unsafe impl Sync for ObsWeakSource {}

pub trait VideoSourceDefinition {
    type Source: VideoSource;
    fn create(&self, settings: &Data, source: &mut ObsSource) -> Self::Source;
    fn get_defaults(&self, _settings: &mut Data) {}
}

pub trait VideoSource {
    fn update(&mut self, _settings: &Data) {}
    fn get_width(&self) -> u32;
    fn get_height(&self) -> u32;
    fn video_tick(&mut self, _seconds: f32) {}
    fn video_render(&mut self) {}
    fn get_properties(&self) -> Properties {
        Properties::new()
    }
}

pub fn register_source<D: 'static>(id: &str, name: &str, definition: D)
where
    D: VideoSourceDefinition,
{
    let type_data = Box::new(ffi::SourceDefinition {
        // make sure `id` lives as long as our registration.
        // OBS does *not* copy it.
        id: CString::new(id).unwrap(),
        name: CString::new(name).unwrap(),
        inner: definition,
    });
    unsafe {
        let mut si: libobs::obs_source_info = mem::zeroed();
        si.id = type_data.id.as_ptr();
        si.type_data = Box::into_raw(type_data) as *mut c_void;
        si.free_type_data = Some(ffi::source_free_type_data::<D>);
        si.type_ = libobs::obs_source_type_OBS_SOURCE_TYPE_INPUT;
        si.output_flags = libobs::OBS_SOURCE_VIDEO;
        si.get_name = Some(ffi::source_get_name::<D>);
        si.create = Some(ffi::source_create::<D>);
        si.destroy = Some(ffi::source_destroy::<D::Source>);
        si.get_defaults2 = Some(ffi::source_get_defaults::<D>);
        si.get_properties = Some(ffi::source_get_properties::<D::Source>);
        si.update = Some(ffi::source_update::<D::Source>);
        si.get_width = Some(ffi::source_get_width::<D::Source>);
        si.get_height = Some(ffi::source_get_height::<D::Source>);
        si.video_tick = Some(ffi::source_video_tick::<D::Source>);
        si.video_render = Some(ffi::source_video_render::<D::Source>);
        libobs::obs_register_source_s(
            &si as *const libobs::obs_source_info,
            mem::size_of::<libobs::obs_source_info>(),
        );
    }
}

pub fn source_create_private(
    id: &str,
    name: Option<&str>,
    settings: Option<&Data>,
) -> Option<ObsSource> {
    let id = CString::new(id).unwrap();
    let name = name.map(|n| CString::new(n).unwrap());
    unsafe {
        let source = libobs::obs_source_create_private(
            id.as_ptr(),
            name.map(|n| n.as_ptr()).unwrap_or(ptr::null()),
            settings.map(|s| s.as_raw()).unwrap_or(ptr::null_mut()),
        );
        if source.is_null() {
            None
        } else {
            Some(ObsSource(source))
        }
    }
}

pub fn get_source_defaults(id: &str) -> Option<Data> {
    unsafe {
        let data = libobs::obs_get_source_defaults(CString::new(id).unwrap().as_ptr());
        if data.is_null() {
            None
        } else {
            Some(Data::from_raw(data))
        }
    }
}
