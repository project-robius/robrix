//! A file previewer modal widget that displays file metadata and previews.
//!
//! This widget handles FilePreviewerAction to show and hide the previewer modal.
//! When the user confirms the upload, it sends a `TimelineUpdate::FileUploadConfirmed`
//! through the timeline-specific channel to ensure the upload is associated with
//! the correct room/timeline.
//! ```

use makepad_widgets::*;
use makepad_widgets::image_cache::{ImageBuffer, ImageError};
use matrix_sdk::attachment::Thumbnail;

use crate::home::room_screen::TimelineUpdate;

/// Decodes image data into an `ImageBuffer` for rendering.
///
/// Supports PNG and JPEG formats only. Other formats will return an error.
///
/// # Errors
/// Returns `ImageError::UnsupportedFormat` if the image format is not PNG or JPEG,
/// or if the format cannot be detected from the data.
fn load_image_from_bytes(data: &[u8]) -> Result<ImageBuffer, ImageError> {
    match imghdr::from_bytes(data) {
        Some(imghdr::Type::Png) => ImageBuffer::from_png(data),
        Some(imghdr::Type::Jpeg) => ImageBuffer::from_jpg(data),
        Some(_) | None => Err(ImageError::UnsupportedFormat),
    }
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    FILE_UPLOAD_MODAL_BORDER_RADIUS: 6.0

    pub FileUploadModal = {{FileUploadModal}}<RoundedView> {
        width: Fill { max: 1000 }
        height: Fit
        align: {x: 0.5, y: 0.5}
        margin: 40,

        flow: Down
        padding: {top: 20, right: 25, bottom: 20, left: 25}

        show_bg: true
        draw_bg: {
            color: (COLOR_PRIMARY)
            border_radius: (FILE_UPLOAD_MODAL_BORDER_RADIUS)
            border_size: 0.0
        }
        // Make this a ScrollYView
        scroll_bars: <ScrollBars> {
            show_scroll_x: false, show_scroll_y: true,
            scroll_bar_y: {drag_scrolling: true}
        }
        // Title and close button
        title_view = <View> {
            width: Fill, height: Fit,
            flow: Right,
            align: {y: 0.5}

            title = <Label> {
                width: Fill, height: Fit,
                draw_text: {
                    text_style: <TITLE_TEXT>{font_size: 16},
                    color: #000
                }
                text: "Upload File"
            }
        }

        // File metadata section
        metadata_view = <View> {
            width: Fill, height: Fit,
            flow: Right,
            align: {y: 0.5}
            margin: {top: 10, bottom: 10}

            // Document icon (visible only for non-image files)
            document_view = <View> {
                visible: false,
                width: Fit, height: Fit,
                align: {x: 0.5, y: 0.5}
                margin: {right: 10}

                file_icon = <Icon> {
                    draw_icon: {
                        svg_file: (ICON_FILE),
                        color: #999,
                    }
                    icon_walk: { width: 24, height: 24 }
                }
            }

            filename_text = <Label> {
                width: Fill, height: Fit,
                draw_text: {
                    text_style: <REGULAR_TEXT>{font_size: 13},
                    color: (COLOR_TEXT),
                    wrap: Word
                }
            }
        }

        // Image preview (visible only for image files)
        image_view = <View> {
            width: Fill, height: Fit { max: 400 },
            flow: Down,
            align: {x: 0.5, y: 0.5}
            margin: {top: 5, bottom: 5}

            preview_image = <Image> {
                width: Fill, height: 300,
                fit: Smallest,
            }
        }

        // Action buttons
        buttons_view = <View> {
            width: Fill, height: Fit,
            flow: Right,
            margin: {top: 15}
            align: {x: 0.5, y: 0.5}
            spacing: 20

            cancel_button = <RobrixIconButton> {
                width: 100,
                align: {x: 0.5, y: 0.5}
                padding: 15,
                icon_walk: {width: 0, height: 0, margin: 0}

                draw_bg: {
                    border_size: 1.0
                    border_color: (COLOR_BG_DISABLED),
                    color: (COLOR_SECONDARY)
                }
                draw_text: {
                    color: (COLOR_TEXT),
                }
                text: "Cancel"
            }

            upload_button = <RobrixIconButton> {
                width: 100
                align: {x: 0.5, y: 0.5}
                padding: 15,
                icon_walk: {width: 0, height: 0, margin: 0}

                draw_bg: {
                    border_size: 1.0
                    border_color: (COLOR_ACTIVE_PRIMARY_DARKER),
                    color: (COLOR_ACTIVE_PRIMARY)
                }
                draw_text: {
                    color: (COLOR_PRIMARY),
                }
                text: "Upload"
            }
        }
    }
}

/// Actions emitted by the `FileUploadModal` widget.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, DefaultNone)]
pub enum FilePreviewerAction {
    /// Display the FileUploadModal widget with the given file data.
    /// The file data includes the timeline update sender for sending confirmation.
    Show(FileData),
    /// Hide the FileUploadModal widget.
    Hide,
    /// No action.
    None,
}

/// Data for a file to be uploaded, including metadata, optional thumbnail,
/// and the timeline update sender to associate the upload with a specific timeline.
pub struct FileData {
    /// Metadata about the file (path, size, MIME type).
    pub metadata: FilePreviewerMetaData,
    /// Optional thumbnail for image files.
    pub thumbnail: Option<Thumbnail>,
    /// The sender to notify the timeline when upload is confirmed.
    pub timeline_update_sender: crossbeam_channel::Sender<TimelineUpdate>,
}

impl Clone for FileData {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            thumbnail: self.thumbnail.as_ref().map(|t| Thumbnail {
                data: t.data.clone(),
                content_type: t.content_type.clone(),
                height: t.height,
                width: t.width,
                size: t.size,
            }),
            timeline_update_sender: self.timeline_update_sender.clone(),
        }
    }
}

impl std::fmt::Debug for FileData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileData")
            .field("metadata", &self.metadata)
            .field("thumbnail", &self.thumbnail.as_ref().map(|_| "..."))
            .field("timeline_update_sender", &"<channel>")
            .finish()
    }
}

impl FileData {
    /// Creates a new FileData by combining loaded file info with a timeline sender.
    pub fn new(
        loaded: FileLoadedData,
        timeline_update_sender: crossbeam_channel::Sender<TimelineUpdate>,
    ) -> Self {
        Self {
            metadata: loaded.metadata,
            thumbnail: loaded.thumbnail,
            timeline_update_sender,
        }
    }
}

/// Data loaded from a file by a background thread.
/// This is sent through a channel and combined with a timeline sender to create `FileData`.
#[derive(Debug)]
pub struct FileLoadedData {
    /// Metadata about the file (path, size, MIME type).
    pub metadata: FilePreviewerMetaData,
    /// Optional thumbnail for image files.
    pub thumbnail: Option<Thumbnail>,
}

impl Clone for FileLoadedData {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            thumbnail: self.thumbnail.as_ref().map(|t| Thumbnail {
                data: t.data.clone(),
                content_type: t.content_type.clone(),
                height: t.height,
                width: t.width,
                size: t.size,
            }),
        }
    }
}

/// Type alias for the receiver that gets loaded file data from a background thread.
pub type FileLoadReceiver = std::sync::mpsc::Receiver<Option<FileLoadedData>>;

/// A widget that previews files by displaying metadata and content based on file type.
#[derive(Live, Widget, LiveHook)]
pub struct FileUploadModal {
    #[redraw] #[deref] view: View,
    #[rust] file_type: FileType,
    #[rust] file_data: Option<FileData>,
}

impl FileUploadModal {
    /// Sets the file content to preview, including metadata and image/document display.
    /// For images, attempts to decode and display the preview. Falls back to document view on error.
    fn set_content(&mut self, cx: &mut Cx, file_data: FileData) {
        self.file_data = Some(file_data.clone());
        self.set_metadata(cx, &file_data);
        if let Some(thumbnail) = &file_data.thumbnail {
            if let Ok(image_buffer) = load_image_from_bytes(&thumbnail.data) {
                let image_ref = self.view.image(ids!(image_view.preview_image));
                let texture = image_buffer.into_new_texture(cx);
                image_ref.set_texture(cx, Some(texture));

                self.view(ids!(image_view)).set_visible(cx, true);
                self.view(ids!(metadata_view.document_view)).set_visible(cx, false);
                self.file_type = FileType::Image;
            } else {
                log!("Failed to decode image data, falling back to document view");
                self.show_document(cx);
            }
        } else {
            self.show_document(cx);
        }
        self.redraw(cx);
    }
}

impl Widget for FileUploadModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for FileUploadModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let cancel_button = self.view.button(ids!(buttons_view.cancel_button));

        // Handle closing the modal via close button or cancel button
        let cancel_clicked = cancel_button.clicked(actions);
        if  cancel_clicked ||
            actions.iter().any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            // If the modal was dismissed by clicking outside of it, we MUST NOT emit
            // a FilePreviewerAction::Hide action, as that would cause
            // an infinite action feedback loop.
            if cancel_clicked {
                cx.action(FilePreviewerAction::Hide);
            }
            self.file_data = None;
            return;
        }

        if self.view.button(ids!(buttons_view.upload_button)).clicked(actions) {
            if let Some(file_data) = self.file_data.take() {
                // Send the file upload confirmation through the timeline-specific channel
                // included in the file data.
                let _ = file_data.timeline_update_sender.send(TimelineUpdate::FileUploadConfirmed(file_data.clone()));
                SignalToUI::set_ui_signal();
            }
            cx.action(FilePreviewerAction::Hide);
            return;
        }

        for action in actions {
            if let Some(FilePreviewerAction::Show(file_data)) = action.downcast_ref() {
                self.set_content(cx, file_data.clone());
                continue;
            }
        }
    }
}

impl FileUploadModal {
    /// Sets the displayed file metadata (filename and formatted size).
    pub fn set_metadata(&mut self, cx: &mut Cx, file_data: &FileData) {
        let formatted_size = crate::utils::format_file_size(file_data.metadata.file_size);
        let filename = file_data.metadata.file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let display_text = format!("{} - ({})", filename, formatted_size);
        self.view.label(ids!(metadata_view.filename_text)).set_text(cx, &display_text);
    }

    /// Displays an image preview by calling the provided function to set the image content.
    ///
    /// Falls back to document view if the image setting function returns an error.
    pub fn show_image<F, E>(&mut self, cx: &mut Cx, image_set_function: F) -> Result<(), E>
        where F: FnOnce(&mut Cx, ImageRef) -> Result<(), E>
    {
        let image_ref = self.view.image(ids!(image_view.preview_image));
        match image_set_function(cx, image_ref) {
            Ok(_) => {
                self.file_type = FileType::Image;
                self.view(ids!(image_view)).set_visible(cx, true);
                self.view(ids!(metadata_view.document_view)).set_visible(cx, false);
                Ok(())
            }
            Err(error) => {
                // Fall back to document view when image cannot be loaded
                self.show_document(cx);
                Err(error)
            }
        }
    }

    /// Displays the document view with a file icon.
    /// Used for non-image files or when image preview fails.
    pub fn show_document(&mut self, cx: &mut Cx) {
        self.file_type = FileType::Document;
        self.view(ids!(metadata_view.document_view)).set_visible(cx, true);
        self.view(ids!(image_view)).set_visible(cx, false);
    }

    /// Returns the current file type being displayed.
    pub fn file_type(&self) -> FileType {
        self.file_type
    }
}

impl FileUploadModalRef {
    /// See [FileUploadModal::set_metadata()].
    pub fn set_metadata(&self, cx: &mut Cx, file_data: &FileData) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_metadata(cx, file_data);
        }
    }

    /// See [FileUploadModal::show_image()].
    pub fn show_image<F, E>(&self, cx: &mut Cx, image_set_function: F) -> Result<(), E>
        where F: FnOnce(&mut Cx, ImageRef) -> Result<(), E>
    {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_image(cx, image_set_function)
        } else {
            Ok(())
        }
    }

    /// See [FileUploadModal::show_document()].
    pub fn show_document(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_document(cx);
        }
    }

    /// See [FileUploadModal::file_type()].
    pub fn file_type(&self) -> FileType {
        if let Some(inner) = self.borrow() {
            inner.file_type()
        } else {
            FileType::Document
        }
    }
}

/// The type of file being displayed in the previewer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    #[default]
    Document,
    Image,
}

/// Metadata for a file to be previewed.
#[derive(Debug, Clone)]
pub struct FilePreviewerMetaData {
    /// MIME type of the file
    pub mime: mime_guess::Mime,
    /// File size in bytes
    pub file_size: u64,
    /// Path to the original file
    pub file_path: std::path::PathBuf,
}
