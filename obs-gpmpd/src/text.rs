use {Client, ClientAccess, ClientId};
use futures::future;
use obs::{self, Data, ObsSource, Properties, VideoSource, VideoSourceDefinition};
use std::borrow::Cow;
use std::sync::Arc;
use tera::{self, Tera};

#[cfg(windows)]
const TEXT_TYPE: &str = "text_gdiplus";
#[cfg(not(windows))]
const TEXT_TYPE: &str = "text_ft2_source";

fn create_child_settings(settings: &Data) -> Data {
    let is_playing = settings.get_bool("is_playing");

    let text = if is_playing {
        let artist = settings.get_string("artist");
        let album = settings.get_string("album");
        let title = settings.get_string("title");
        let template = settings.get_string("text");
        let template = template.as_ref().map(|s| s.as_str()).unwrap_or("");

        let mut context = tera::Context::new();
        context.add("artist", &artist);
        context.add("album", &album);
        context.add("title", &title);
        Cow::Owned(
            Tera::one_off(template, &context, false).unwrap_or_else(|e| format!("error: {:?}", e)),
        )
    } else {
        Cow::Borrowed("")
    };

    let mut child_settings = Data::new();
    child_settings.apply(settings);
    child_settings.set_string("text", &text);
    child_settings.set_bool("read_from_file", false);
    child_settings.set_bool("chatlog", false);
    child_settings
}

pub(super) struct NowPlayingSourceDefinition {
    client_access: Arc<ClientAccess>,
}

impl NowPlayingSourceDefinition {
    pub fn new(client_access: &Arc<ClientAccess>) -> Self {
        Self {
            client_access: client_access.clone(),
        }
    }
}

pub(super) struct NowPlayingSource {
    _client: Option<Client>,
    text: Option<ObsSource>,
}

impl VideoSourceDefinition for NowPlayingSourceDefinition {
    type Source = NowPlayingSource;
    fn create(&self, settings: &Data, source: &mut ObsSource) -> Self::Source {
        let update_source = source.get_weak_source();
        NowPlayingSource {
            _client: self.client_access
                .client(&ClientId::Text(source.get_name()), move |s, _| {
                    if let Some(source) = update_source.upgrade() {
                        let mut data = Data::new();
                        data.set_string(
                            "artist",
                            s.track
                                .as_ref()
                                .and_then(|s| s.artist.as_ref())
                                .map(|s| s.as_str())
                                .unwrap_or(""),
                        );
                        data.set_string(
                            "album",
                            s.track
                                .as_ref()
                                .and_then(|s| s.album.as_ref())
                                .map(|s| s.as_str())
                                .unwrap_or(""),
                        );
                        data.set_string(
                            "title",
                            s.track
                                .as_ref()
                                .and_then(|s| s.title.as_ref())
                                .map(|s| s.as_str())
                                .unwrap_or(""),
                        );
                        data.set_bool("is_playing", s.is_playing);
                        source.update(&data);
                    }
                    future::ok(())
                })
                .map_err(|e| error!("failed to get client: {:?}", e))
                .ok(),
            text: obs::source_create_private(
                TEXT_TYPE,
                Some("gpmdp-text"),
                Some(&create_child_settings(settings)),
            ),
        }
    }
    fn get_defaults(&self, settings: &mut Data) {
        if let Some(text_settings) = obs::get_source_defaults(TEXT_TYPE) {
            // this does not seem to work
            settings.apply(&text_settings);
        }
        settings.set_default_string("text", "{{title}}\n{{artist}} - {{album}}");
        settings.set_default_string("artist", "[artist]");
        settings.set_default_string("album", "[album]");
        settings.set_default_string("title", "[title]");
    }
}

impl VideoSource for NowPlayingSource {
    fn get_properties(&self) -> Properties {
        let props = match self.text {
            Some(ref text) => text.get_properties(),
            None => Properties::new(),
        };
        if let Some(mut text) = props.get_property("text") {
            text.set_description(&::obs_module_text("Template"));
        }
        if let Some(mut read_from_file) = props.get_property("read_from_file") {
            read_from_file.set_visible(false);
        }
        if let Some(mut chatlog) = props.get_property("chatlog") {
            chatlog.set_visible(false);
        }
        props
    }
    fn update(&mut self, settings: &Data) {
        if let Some(ref mut text) = self.text {
            text.update(&create_child_settings(settings));
        }
    }
    fn get_width(&self) -> u32 {
        match self.text {
            Some(ref text) => text.get_width(),
            None => 0,
        }
    }
    fn get_height(&self) -> u32 {
        match self.text {
            Some(ref text) => text.get_height(),
            None => 0,
        }
    }
    fn video_render(&mut self) {
        if let Some(ref text) = self.text {
            text.video_render();
        }
    }
}
