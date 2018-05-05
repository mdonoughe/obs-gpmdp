#![macro_use]

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => ($crate::obs::blog(400, ::std::fmt::format(format_args!($($arg)*))));
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => ($crate::obs::blog(300, ::std::fmt::format(format_args!($($arg)*))));
}

#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => ($crate::obs::blog(200, ::std::fmt::format(format_args!($($arg)*))));
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => ($crate::obs::blog(100, ::std::fmt::format(format_args!($($arg)*))));
}

#[macro_export]
macro_rules! obs_declare_module {
    ($type:ty, $name:expr, $description:expr) => {
        static mut OBS_MODULE_POINTER: ::std::option::Option<*mut $crate::obs::obs_module_t> =
            ::std::option::Option::None;
        const OBS_MODULE_NAME: &'static str = concat!($name, "\0");
        const OBS_MODULE_DESCRIPTION: &'static str = concat!($description, "\0");
        static mut MODULE_VALUE: ::std::option::Option<$type> = ::std::option::Option::None;

        #[no_mangle]
        pub unsafe extern "C" fn obs_module_set_pointer(module: *mut $crate::obs::obs_module_t) -> () {
            OBS_MODULE_POINTER = ::std::option::Option::Some(module);
        }

        #[no_mangle]
        pub unsafe extern "C" fn obs_module_ver() -> u32 {
            (($crate::obs::LIBOBS_API_MAJOR_VER as u32) << 24)
            | (($crate::obs::LIBOBS_API_MINOR_VER as u32) << 16)
            | $crate::obs::LIBOBS_API_PATCH_VER as u32
        }

        #[no_mangle]
        pub unsafe extern "C" fn obs_module_name() -> *const ::std::os::raw::c_char {
            OBS_MODULE_NAME.as_bytes().as_ptr() as *const ::std::os::raw::c_char
        }

        #[no_mangle]
        pub unsafe extern "C" fn obs_module_description() -> *const ::std::os::raw::c_char {
            OBS_MODULE_DESCRIPTION.as_bytes().as_ptr() as *const ::std::os::raw::c_char
        }

        #[no_mangle]
        pub unsafe extern "C" fn obs_module_load() -> bool {
            MODULE_VALUE = <$type as ::obs::Module<$type>>::load();
            MODULE_VALUE.is_some()
        }

        #[no_mangle]
        pub unsafe extern "C" fn obs_module_unload() -> () {
            MODULE_VALUE = None;
        }
    };
    ($type:ty, $name:expr, $description:expr, $author:expr) => {
        obs_declare_module!($type, $name, $description);

        const OBS_MODULE_AUTHOR: &'static str = concat!($author, "\0");

        #[no_mangle]
        pub unsafe extern "C" fn obs_module_author() -> *const ::std::os::raw::c_char {
            OBS_MODULE_AUTHOR.as_bytes().as_ptr() as *const ::std::os::raw::c_char
        }
    }
}

#[macro_export]
macro_rules! obs_module_use_default_locale {
    ($locale:expr) => {
        const OBS_MODULE_DEFAULT_LOCALE: &'static str = concat!($locale, "\0");

        lazy_static! {
            static ref OBS_MODULE_LOOKUP:
                ::std::sync::RwLock<::std::option::Option<$crate::obs::Lookup>>
                = ::std::sync::RwLock::new(::std::option::Option::None);
        }

        pub fn obs_module_text(val: &str) -> ::std::borrow::Cow<str> {
            let guard = OBS_MODULE_LOOKUP.read().unwrap();
            match *guard {
                ::std::option::Option::Some(ref lookup) => match lookup.getstr(val) {
                    ::std::option::Option::Some(out) => ::std::borrow::Cow::Owned(out),
                    ::std::option::Option::None => ::std::borrow::Cow::Borrowed(val)
                },
                ::std::option::Option::None => ::std::borrow::Cow::Borrowed(val)
            }
        }

        pub fn obs_module_get_string(val: &str) -> String {
            let guard = OBS_MODULE_LOOKUP.read().unwrap();
            guard.as_ref().unwrap().getstr(val).unwrap()
        }

        #[no_mangle]
        pub unsafe extern "C" fn obs_module_set_locale(locale: *const ::std::os::raw::c_char) {
            let mut guard = OBS_MODULE_LOOKUP.write().unwrap();
            *guard = ::std::option::Option::Some($crate::obs::load_locale(
                OBS_MODULE_POINTER.unwrap(),
                OBS_MODULE_DEFAULT_LOCALE.as_bytes().as_ptr() as *const ::std::os::raw::c_char,
                locale))
        }

        #[no_mangle]
        pub unsafe extern "C" fn obs_module_free_locale() {
            let mut guard = OBS_MODULE_LOOKUP.write().unwrap();
            *guard = ::std::option::Option::None;
        }
    };
}
