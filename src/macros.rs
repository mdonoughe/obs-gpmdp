#![macro_use]

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
