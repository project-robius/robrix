use std::sync::Arc;

use makepad_widgets::*;
use matrix_sdk::ruma::events::room::{ImageInfo, MediaSource};

use crate::{media_cache::{MediaCache, MediaCacheEntry}, utils};

use super::text_or_image::TextOrImageRef;

#[derive(Clone, DefaultNone, Debug)]
pub enum ImageViewerAction {
    Open,
    Close,
    None
}


live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub ImageViewer = {{ImageViewer}} {
        width: Fit
        height: Fit
        visible: false

        flow: Overlay

        align: {x: 1., y: 0.}

        close_button = <RobrixIconButton> {
            enabled: false,
            padding: {left: 15, right: 15}
            draw_icon: {
                svg_file: (ICON_CLOSE)
                color: (COLOR_ACCEPT_GREEN),
            }
            icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

            draw_bg: {
                border_color: (COLOR_ACCEPT_GREEN),
                color: #f0fff0 // light green
            }
        }
        <Image> {
            fit: Stretch,
            width: Fill, height: Fill,
            source: (IMG_DEFAULT_AVATAR),
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ImageViewer {
    #[deref] view: View,
    #[rust] image: Arc<[u8]>
}

impl Widget for ImageViewer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for ImageViewer {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for action in actions {
            match action.downcast_ref() {
                Some(ImageViewerAction::Open) => {

                },
                Some(ImageViewerAction::Close) => {

                },
                Some(ImageViewerAction::None) | None => {}
            }
        }
    }
}

impl ImageViewer {

}

/// We fetch thumbnail of the media in `populate_image_message_content` in `room_screen.rs`.
///
/// Now We fetch origin of the media.
pub fn fetch_origin_media_source(
    cx: &mut Cx2d,
    text_or_image_ref: &TextOrImageRef,
    image_info_source: Option<(Option<ImageInfo>, MediaSource)>,
    media_cache: &mut MediaCache
)
-> Arc<[u8]>
{
    match image_info_source.map(|(_, media_source)| media_source ) {
            Some(MediaSource::Plain(mxc_uri)) => {
                // Now that we've obtained thumbnail of the image URI and its metadata.
                // Let's try to fetch it.
                match media_cache.try_get_media_or_fetch(mxc_uri.clone(), None) {
                    MediaCacheEntry::Loaded(data) => {
                        let show_image_result = text_or_image_ref.show_image(|img| {
                            utils::load_png_or_jpg(&img, cx, &data)
                                .map(|()| img.size_in_pixels(cx).unwrap_or_default())
                        });
                        if let Err(e) = show_image_result {
                            let err_str = format!("{body}\n\nFailed to display image: {e:?}");
                            error!("{err_str}");
                            text_or_image_ref.show_text(&err_str);
                        }

                        // We're done drawing thumbnail of the image message content, so mark it as fully drawn.
                        true
                    }
                    MediaCacheEntry::Requested => {
                        text_or_image_ref.show_text(format!("{body}\n\nFetching image from {:?}", mxc_uri));
                        // Do not consider this thumbnail as being fully drawn, as we're still fetching it.
                        false
                    }
                    MediaCacheEntry::Failed => {
                        text_or_image_ref
                            .show_text(format!("{body}\n\nFailed to fetch image from {:?}", mxc_uri));
                        // For now, we consider this as being "complete". In the future, we could support
                        // retrying to fetch thumbnail of the image on a user click/tap.
                        true
                    }
                }
            }
            Some(MediaSource::Encrypted(encrypted)) => {
                text_or_image_ref.show_text(format!(
                    "{body}\n\n[TODO] fetch encrypted image at {:?}",
                    encrypted.url
                ));
                // We consider this as "fully drawn" since we don't yet support encryption.
                true
            }
            Some(None) | None => {
                text_or_image_ref.show_text("{body}\n\nImage message had no source URL.");
                true
            }
    }
}
