extern crate bindgen;
#[cfg(windows)]
extern crate cc;
#[cfg(windows)]
extern crate regex;
#[cfg(windows)]
extern crate winreg;

#[cfg(windows)]
mod build_win;

#[cfg(windows)]
use build_win::find_windows_obs_lib;

use bindgen::callbacks::{MacroParsingBehavior, ParseCallbacks};
use std::env;
use std::path::PathBuf;

#[cfg(not(windows))]
fn find_windows_obs_lib() {}

#[derive(Debug)]
struct MacroCallback();

impl ParseCallbacks for MacroCallback {
    fn will_parse_macro(&self, name: &str) -> MacroParsingBehavior {
        match name {
            "FP_ZERO" | "FP_SUBNORMAL" | "FP_NORMAL" | "FP_INFINITE" | "FP_NAN" => {
                MacroParsingBehavior::Ignore
            }
            _ => MacroParsingBehavior::Default,
        }
    }
}

fn main() {
    // Tell cargo to tell rustc to link the system obs
    // shared library.
    println!("cargo:rustc-link-lib=dylib=obs");

    find_windows_obs_lib();

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        .parse_callbacks(Box::new(MacroCallback()))
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
