# obs-gpmdp

[OBS Studio] plugin for displaying information from [Google Play Music Desktop Player]

## Usage

### Dependencies

- [OBS Studio] 21
- [Google Play Music Desktop Player] 4 with Playback API enabled in Desktop Settings

### Installation

Copy gpmdp.dll (or gpmdp.so or gpmdp.dylib) into the OBS Studio plugin directory. On Windows it'll be something like "C:\Program Files (x86)\obs-studio\obs-plugins\64bit".

### Usage

obs-gpmdp adds two new sources to OBS. You will want to have something playing while you customize your scene because everything is supposed to autohide when nothing is playing.

#### GPMDP Album Art

GPMDP Album Art is simply the album art for the currently playing track. It should work like an image source.

#### GPMDP Now Playing

GPMDP Now Playing is a custom text source that can be used to display information about the currently playing track. It takes most of the usual text source properties, except that it takes a template instead of static text.

The templates are rendered by [Tera]. Basically, you put placeholders inside double curly braces like `{{title}}` and they get replaced with the values from GPMDP.

The allowable placeholders are:

- `{{artist}}`: displays the name of the artist
- `{{album}}`: displays the name of the album
- `{{title}}`: displays the title of the track

Here are some simple templates that should work:

- `{{title}}`
- `{{artist}} - {{album}}`

Tera has some other features too. Most of them probably aren't that useful in this case, but you can do things like `{{title | upper}}` if you want the title to appear in all capitals. See the [template documentation](https://tera.netlify.com/docs/templates/#templates) for more information.

## Development

obs-gpmdp is implemented as two [Rust] crates.

libobs-sys is just an API definition for OBS built using [rust-bindgen].

obs-gpmdp is the plugin itself. Inside obs-gpmdp the obs module tries to be a generic Rust plugin API for OBS, but it's not documented and really only contains the parts required for obs-gpmdp.

### Building

obs-gpmdp is built using [Cargo] which should be installed if you follow the [Rust installation instructions](https://www.rust-lang.org/en-US/install.html).

On Windows you should only need a Rust build environment because there is a copy of obs.lib in the repository. On other systems you may need to set environment variables so the linker can find obs.

[OBS Studio]: https://obsproject.com/
[Google Play Music Desktop Player]: https://www.googleplaymusicdesktopplayer.com/
[Tera Templates]: https://tera.netlify.com/
[Rust]: https://www.rust-lang.org/en-US/
[rust-bindgen]: https://rust-lang-nursery.github.io/rust-bindgen/
[Cargo]: https://doc.rust-lang.org/cargo/guide/
