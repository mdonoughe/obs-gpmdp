mod callback;
mod data;
mod log;
mod lookup;
mod properties;
mod source;
mod texture;

use std::os::raw::c_char;
use libobs;

pub use self::callback::execute_main_render_callback;
pub use self::data::Data;
pub use libobs::{obs_module_t, obs_text_type, LIBOBS_API_MAJOR_VER, LIBOBS_API_MINOR_VER,
                 LIBOBS_API_PATCH_VER};
pub use self::log::blog;
pub use self::lookup::Lookup;
pub use self::properties::{Properties, Property};
pub use self::source::{get_source_defaults, register_source, source_create_private, ObsSource,
                       VideoSource, VideoSourceDefinition};
pub use self::texture::Texture;

pub trait Module<T>
where
    T: Module<T>,
{
    fn load() -> Option<T>;
}

pub unsafe fn load_locale(
    module: *mut obs_module_t,
    default_locale: *const c_char,
    locale: *const c_char,
) -> Lookup {
    Lookup::from_raw(libobs::obs_module_load_locale(
        module,
        default_locale,
        locale,
    ))
}

// we don't use this pointer value but it prevents Send
pub struct GraphicsHandle(*mut libobs::graphics_t);

impl Drop for GraphicsHandle {
    fn drop(&mut self) {
        unsafe {
            libobs::obs_leave_graphics();
        }
    }
}

pub fn enter_graphics() -> GraphicsHandle {
    unsafe {
        libobs::obs_enter_graphics();
        GraphicsHandle(libobs::gs_get_context())
    }
}
