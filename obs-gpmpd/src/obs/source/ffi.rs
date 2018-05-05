use std::ffi::CString;
use std::mem;
use std::os::raw::{c_char, c_void};
use libobs;
use super::{Data, ObsSource, VideoSource, VideoSourceDefinition};

pub(super) struct SourceDefinition<D>
where
    D: VideoSourceDefinition,
{
    pub id: CString,
    pub name: CString,
    pub inner: D,
}

pub(super) unsafe extern "C" fn source_get_name<D>(data: *mut c_void) -> *const c_char
where
    D: VideoSourceDefinition,
{
    let data = &*(data as *mut SourceDefinition<D>);
    data.name.as_ptr()
}

pub(super) unsafe extern "C" fn source_free_type_data<D>(data: *mut c_void)
where
    D: VideoSourceDefinition,
{
    mem::drop(Box::from_raw(data as *mut SourceDefinition<D>))
}

pub(super) unsafe extern "C" fn source_create<D>(
    settings: *mut libobs::obs_data,
    source: *mut libobs::obs_source,
) -> *mut c_void
where
    D: VideoSourceDefinition,
{
    let data = &*((&*source).info.type_data as *mut SourceDefinition<D>);

    // increment because our wrappers are going to decrement on drop
    libobs::obs_data_addref(settings);
    libobs::obs_source_addref(source);
    Box::into_raw(Box::new(
        data.inner
            .create(&Data::from_raw(settings), &mut ObsSource(source)),
    )) as *mut c_void
}

pub(super) unsafe extern "C" fn source_destroy<S>(source: *mut c_void) {
    mem::drop(Box::from_raw(source as *mut S))
}

pub(super) unsafe extern "C" fn source_get_defaults<D>(
    data: *mut c_void,
    settings: *mut libobs::obs_data,
) where
    D: VideoSourceDefinition,
{
    let data = &*(data as *mut SourceDefinition<D>);
    // increment because our wrappers are going to decrement on drop
    libobs::obs_data_addref(settings);
    data.inner.get_defaults(&mut Data::from_raw(settings));
}

pub(super) unsafe extern "C" fn source_get_properties<S>(
    source: *mut c_void,
) -> *mut libobs::obs_properties_t
where
    S: VideoSource,
{
    let source = &mut *(source as *mut S);
    let properties = source.get_properties();
    properties.to_ptr()
}

pub(super) unsafe extern "C" fn source_update<S>(
    source: *mut c_void,
    settings: *mut libobs::obs_data,
) where
    S: VideoSource,
{
    let source = &mut *(source as *mut S);
    // increment because our wrappers are going to decrement on drop
    libobs::obs_data_addref(settings);
    source.update(&Data::from_raw(settings));
}

pub(super) unsafe extern "C" fn source_get_width<S>(source: *mut c_void) -> u32
where
    S: VideoSource,
{
    let source = &mut *(source as *mut S);
    source.get_width()
}

pub(super) unsafe extern "C" fn source_get_height<S>(source: *mut c_void) -> u32
where
    S: VideoSource,
{
    let source = &mut *(source as *mut S);
    source.get_height()
}

pub(super) unsafe extern "C" fn source_video_tick<S>(source: *mut c_void, seconds: f32)
where
    S: VideoSource,
{
    let source = &mut *(source as *mut S);
    source.video_tick(seconds);
}

pub(super) unsafe extern "C" fn source_video_render<S>(
    source: *mut c_void,
    _effect: *mut libobs::gs_effect,
) where
    S: VideoSource,
{
    let source = &mut *(source as *mut S);
    source.video_render();
}
