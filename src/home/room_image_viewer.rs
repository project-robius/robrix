//! Room Image Viewer components for displaying images in full-screen modal.
//!
//! The main component is `RoomImageViewerDetail` which provides an overlay showing:
//! - Sender information (avatar, username, timestamp) 
//! - Image metadata (filename, file size)
//!
//! Used with the `ImageViewer` widget from `shared/image_viewer.rs` for the full viewing experience.

use makepad_widgets::*;

use matrix_sdk::{media::MediaFormat, ruma::{MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedRoomId, OwnedUserId, OwnedMxcUri}};
use matrix_sdk_ui::timeline::{Profile, TimelineDetails};
use reqwest::StatusCode;
use crate::{media_cache::{MediaCache, MediaCacheEntry}, shared::{avatar::AvatarWidgetExt, image_viewer::{ImageViewerAction, ImageViewerError, ImageViewerWidgetExt, LoadState, image_viewer_error_to_string}, timestamp::TimestampWidgetExt}, utils::unix_time_millis_to_datetime};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::timestamp::Timestamp;
    use crate::shared::image_viewer::ImageViewer;

    pub RoomImageViewerDetail = {{RoomImageViewerDetail}} {
        width: Fill, height: Fill
        flow: Right

        top_left_container = <View> {
            width: 150, height: Fit,
            flow: Right,
            spacing: 10,
            margin: {left: 20, top: 40}
            align: { y: 0.5 }

            avatar = <Avatar> {
                width: 40, height: 40,
            }

            content = <View> {
                width: Fill, height: Fit,
                flow: Down,
                spacing: 4,

                username = <Label> {
                    width: Fill, height: Fit,
                    draw_text: {
                        text_style: <REGULAR_TEXT>{font_size: 14},
                        color: (COLOR_TEXT)
                    }
                }
                timestamp_view = <View> {
                    width: Fill, height: Fit
                    timestamp = <Timestamp> {
                        width: Fill, height: Fit,
                        margin: { left: 5 }
                    }
                }
            }
        }

        image_name_and_size = <Label> {
            width: Fill, height: Fit,
            margin: {top: 40}
            align: { x: 0.5, }
            draw_text: {
                text_style: <REGULAR_TEXT>{font_size: 14},
                color: (COLOR_TEXT),
                wrap: Word
            }
        }

        empty_right_container = <View> {
            // equal width as the top-left container to keep the image name centered.
            width: 150, height: Fit,
        }
    }
    pub RoomImageViewerFooter = {{RoomImageViewerFooter}} {
        width: Fill, height: 50,
        flow: Right
        padding: 10
        align: {x: 0.5, y: 0.8}
        spacing: 10

        image_viewer_loading_spinner_view = <View> {
            width: Fit, height: Fit
            loading_spinner = <LoadingSpinner> {
                width: 40, height: 40,
                draw_bg: {
                    color: (COLOR_PRIMARY)
                    border_size: 3.0,
                }
            }
        }
        image_viewer_forbidden_view = <View> {
            width: Fit, height: Fit
            visible: false
            <Icon> {
                draw_icon: {
                    svg_file: (ICON_FORBIDDEN),
                    color: #ffffff,
                }
                icon_walk: { width: 30, height: 30 }
            }
        }
        image_viewer_status_label = <Label> {
            width: Fit, height: 30,
            text: "Loading image...",
            draw_text: {
                text_style: <REGULAR_TEXT>{font_size: 14},
                color: (COLOR_PRIMARY)
            }
        }
    }
    pub RoomImageViewer = {{RoomImageViewer}} {
        image_viewer = <Modal> {
            content: {
                width: Fill, height: Fill,
                flow: Down
                show_bg: true
                draw_bg: {
                    color: #000
                }
                image_viewer_inner = <ImageViewer> {
                    align: {x: 0.5, y: 0.5}
                    overlay: <View> {
                        image_detail = <RoomImageViewerDetail> {
                            width: Fill, height: Fill,
                        }
                    }
                }
                
                footer = <RoomImageViewerFooter> {}
            }
        } 
    }
}

#[derive(Live, Widget, LiveHook)]
pub struct RoomImageViewer{
    #[deref] view: View,
}

impl Widget for RoomImageViewer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for RoomImageViewer {
    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions) {
        let mut footer = self.view.room_image_viewer_footer(ids!(footer));
        let mut image_viewer_inner = self.view.image_viewer(ids!(image_viewer_inner));
        for action in actions {
            match action.downcast_ref() {
                Some(ImageViewerAction::Show(load_state)) => {
                    match load_state {
                        LoadState::Loading(texture, image_size) => {
                            self.view.modal(ids!(image_viewer)).open(cx);
                            image_viewer_inner.reset(cx);
                            footer.show_loading(cx);
                            footer.apply_over(cx, live!{
                                height: 50
                            });
                            image_viewer_inner.display_using_texture(cx, texture.as_ref().clone(), image_size);
                        }
                        LoadState::Loaded(image_bytes) => {
                            self.view.modal(ids!(image_viewer)).open(cx);
                            image_viewer_inner.display_using_background_thread(cx, image_bytes);
                        }
                        LoadState::FinishedBackgroundDecoding => {
                            footer.hide(cx);
                            // Collapse the footer
                            footer.apply_over(cx, live!{
                                height: 0
                            });
                        }
                        LoadState::Error(error) => {
                            if self.view.modal(ids!(image_viewer)).is_open() {
                                footer.show_error(cx, image_viewer_error_to_string(error));
                                footer.apply_over(cx, live!{
                                    height: 50
                                });
                            }
                        }
                    }
                    
                }
                Some(ImageViewerAction::Hide) => {
                    self.view.modal(ids!(image_viewer)).close(cx);
                    self.view.room_image_viewer_detail(ids!(image_detail)).reset_state(cx);
                }
                _ => {}
            }
        }
    }
}


/// A room image viewer detail widget that displays a user's avatar, username, and message date.
#[derive(Live, LiveHook, Widget)]
struct RoomImageViewerDetail {
    #[deref]
    view: View,
    /// The sender of the message
    #[rust]
    sender: Option<OwnedUserId>,
    /// The profile of the sender
    #[rust]
    sender_profile: Option<TimelineDetails<Profile>>,
    /// The room ID
    #[rust]
    room_id: Option<OwnedRoomId>,
    /// The event ID
    #[rust]
    event_id: Option<OwnedEventId>,
    /// The sender's avatar has been drawn. Will not be drawn again if set to `true`.
    #[rust]
    avatar_drawn: bool,
    /// The name of the image.
    #[rust]
    image_name: String,
    /// The size of the image in bytes.
    #[rust]
    image_size: i32,
}

/// Convert bytes to human-readable file size format
fn format_file_size(bytes: i32) -> String {
    if bytes < 0 {
        return "Unknown size".to_string();
    }

    let bytes = bytes as u64;
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Maximum image name length to be displayed
const MAX_IMAGE_NAME_LENGTH: usize = 50;

/// Truncate image name while preserving file extension
fn truncate_image_name(image_name: &str) -> String {
    let max_length = MAX_IMAGE_NAME_LENGTH;

    if image_name.len() <= max_length {
        return image_name.to_string();
    }

    // Find the last dot to separate name and extension
    if let Some(dot_pos) = image_name.rfind('.') {
        let name_part = &image_name[..dot_pos];
        let extension = &image_name[dot_pos..];

        // Reserve space for "..." and the extension
        let available_length = max_length.saturating_sub(3 + extension.len());

        if available_length > 0 && name_part.len() > available_length {
            format!("{}...{}", &name_part[..available_length], extension)
        } else {
            image_name.to_string()
        }
    } else {
        // No extension found, just truncate the name
        format!("{}...", &image_name[..max_length.saturating_sub(3)])
    }
}

impl Widget for RoomImageViewerDetail {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.avatar_drawn {
            let avatar_ref = self.avatar(ids!(top_left_container.avatar));
            let Some(room_id) = &self.room_id else {
                return DrawStep::done();
            };
            let Some(sender) = &self.sender else {
                return DrawStep::done();
            };
            let (username, avatar_drawn) = avatar_ref.set_avatar_and_get_username(
                cx,
                room_id,
                sender,
                self.sender_profile.as_ref(),
                self.event_id.as_deref(),
            );
            self.label(ids!(top_left_container.username))
                .set_text(cx, &username);
            self.avatar_drawn = avatar_drawn;
            let is_desktop = cx.display_context.is_desktop();
            let truncated_name = truncate_image_name(&self.image_name);
            let human_readable_size = format_file_size(self.image_size);
            let display_text = format!("{} ({})", truncated_name, human_readable_size);
            self.label(ids!(image_name_and_size))
                .set_text(cx, &display_text);
            let empty_right_container_width = if is_desktop { 150 } else { 0 };
            self.view(ids!(empty_right_container)).apply_over(
                cx,
                live! {
                    width: (empty_right_container_width)
                },
            );
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for RoomImageViewerDetail {
    fn handle_action(&mut self, cx: &mut Cx, action: &Action) {
        if let RoomImageViewerDetailAction::SetImageDetail {
            room_id,
            sender,
            sender_profile,
            event_id,
            timestamp_millis,
            image_name,
            image_size,
        } = action.as_widget_action().cast()
        {
            self.room_id = room_id.clone();
            self.sender = sender.clone();
            self.sender_profile = sender_profile.clone();
            self.event_id = event_id.clone();
            self.avatar_drawn = false;
            // Format and display image name and size
            let truncated_name = truncate_image_name(&image_name);
            let human_readable_size = format_file_size(image_size);
            let display_text = format!("{} ({})", truncated_name, human_readable_size);
            self.image_name = image_name;
            self.image_size = image_size;
            self.label(ids!(image_name_and_size))
                .set_text(cx, &display_text);
            if let Some(dt) = unix_time_millis_to_datetime(timestamp_millis) {
                self.view(ids!(timestamp_view)).set_visible(cx, true);
                self.timestamp(ids!(timestamp)).set_date_time(cx, dt);
            }
        }
    }
}

impl RoomImageViewerDetail {
    /// Reset the widget state to its default values
    pub fn reset_state(&mut self, cx: &mut Cx) {
        self.sender = None;
        self.sender_profile = None;
        self.room_id = None;
        self.event_id = None;
        self.avatar_drawn = false;
        self.image_name = String::new();
        self.image_size = 0;

        // Clear the UI elements
        self.label(ids!(top_left_container.username))
            .set_text(cx, "");
        self.label(ids!(image_name_and_size)).set_text(cx, "");
        self.view(ids!(timestamp_view)).set_visible(cx, false);
    }
}

impl RoomImageViewerDetailRef {
    /// See [`RoomImageViewerDetail::reset_state()`]
    pub fn reset_state(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.reset_state(cx);
        }
    }
}

/// Actions handled by the `RoomImageViewerDetail`
#[derive(Debug, Clone, DefaultNone)]
pub enum RoomImageViewerDetailAction {
    /// Set the image detail onto image viewer modal.
    SetImageDetail {
        /// Room ID
        room_id: Option<OwnedRoomId>,
        /// User ID for the sender of the image
        sender: Option<OwnedUserId>,
        /// Profile details for the sender
        sender_profile: Option<TimelineDetails<Profile>>,
        /// Event ID
        event_id: Option<OwnedEventId>,
        /// Timestamp of the message
        timestamp_millis: MilliSecondsSinceUnixEpoch,
        /// Image name
        image_name: String,
        /// Image size in bytes.
        image_size: i32,
    },
    None,
}


/// A room image viewer footer widget that contains loading spinner, error icon, and status label.
#[derive(Live, LiveHook, Widget)]
struct RoomImageViewerFooter {
    #[deref]
    view: View,
}

impl RoomImageViewerFooter {
    /// Shows a loading message in the footer.
    ///
    /// The loading spinner is shown, the error icon is hidden, and the
    /// status label is set to "Loading...".
    pub fn show_loading(&mut self, cx: &mut Cx) {
        self.view.view(ids!(image_viewer_loading_spinner_view)).set_visible(cx, true);
        self.view.label(ids!(image_viewer_status_label)).set_text(cx, "Loading...");
        self.view.view(ids!(image_viewer_forbidden_view)).set_visible(cx, false);
        self.view.apply_over(cx, live!{
            height: 50
        });
    }

    /// Shows an error message in the footer.
    ///
    /// The loading spinner is hidden, the error icon is shown, and the
    /// status label is set to the error message provided.
    pub fn show_error(&mut self, cx: &mut Cx, error: &str) {
        self.view.view(ids!(image_viewer_loading_spinner_view)).set_visible(cx, false);
        self.view.view(ids!(image_viewer_forbidden_view)).set_visible(cx, true);
        self.view.label(ids!(image_viewer_status_label)).set_text(cx, error);
        
    }

    /// Hides all the elements in the footer.
    pub fn hide(&mut self, cx: &mut Cx) {
        self.view.view(ids!(image_viewer_loading_spinner_view)).set_visible(cx, false);
        self.view.view(ids!(image_viewer_forbidden_view)).set_visible(cx, false);
        self.view.label(ids!(image_viewer_status_label)).set_text(cx, "");
    }
}

impl Widget for RoomImageViewerFooter {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomImageViewerFooterRef {

    /// See [`RoomImageViewerFooter::show_loading()`].
    pub fn show_loading(&mut self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_loading(cx);
        }
    }

    /// See [`RoomImageViewerFooter::show_error()`].
    pub fn show_error(&mut self, cx: &mut Cx, error: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_error(cx, error);
        }
    }

    /// See [`RoomImageViewerFooter::hide()`].
    pub fn hide(&mut self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.hide(cx);
        }
    }
}


/// Populates the image viewer modal with the given media content.
///
/// If the media is already cached, it will be immediately displayed.
/// If the media is not cached, it will be fetched from the server.
/// If the media fetch fails, an error message will be displayed.
///
/// This function requires passing in a reference to `Cx`, which isn't used, but acts as a guarantee that this function must only be called by the main UI thread.
pub fn populate_matrix_image_modal(
    cx: &mut Cx,
    mxc_uri: OwnedMxcUri,
    media_cache: &mut MediaCache,
) {
    // Try to get media from cache or trigger fetch
    let media_entry = media_cache.try_get_media_or_fetch(mxc_uri.clone(), MediaFormat::File);

    // Handle the different media states
    match media_entry {
        (MediaCacheEntry::Loaded(data), MediaFormat::File) => {
            cx.action(ImageViewerAction::Show(LoadState::Loaded(data)));
        }
        (MediaCacheEntry::Failed(status_code), MediaFormat::File) => {
            let error = match status_code {
                StatusCode::NOT_FOUND => ImageViewerError::NotFound,
                StatusCode::INTERNAL_SERVER_ERROR => ImageViewerError::ConnectionFailed,
                StatusCode::PARTIAL_CONTENT => ImageViewerError::BadData,
                StatusCode::UNAUTHORIZED => ImageViewerError::Unauthorized,
                StatusCode::REQUEST_TIMEOUT => ImageViewerError::Timeout,
                _ => ImageViewerError::Unknown,
            };
            cx.action(ImageViewerAction::Show(LoadState::Error(error)));
            // Remove failed media entry from cache for MediaFormat::File so as to start all over again from loading Thumbnail.
            media_cache.remove_cache_entry(&mxc_uri, Some(MediaFormat::File));
        }
        _ => {}
    }
}
