//! A file previewer modal widget that displays file metadata and previews.
//!
//! This widget handles FilePreviewerAction to show and hide the previewer modal.
//! It also emits FilePreviewerAction::Upload action to upload the selected file.
//! ```

use std::sync::Arc;

use makepad_widgets::*;
use makepad_widgets::image_cache::{ImageBuffer, ImageError};
use matrix_sdk::attachment::Thumbnail;

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

            close_button = <RobrixIconButton> {
                width: Fit, height: Fit,
                padding: 12,
                spacing: 0
                align: {x: 0.5, y: 0.5}
                icon_walk: {width: 18, height: 18, margin: 0}
                draw_icon: {
                    svg_file: (ICON_CLOSE),
                    color: #666
                }
                draw_bg: {
                    border_size: 0,
                    color: #0000
                }
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
#[derive(Clone, Debug, DefaultNone)]
pub enum FilePreviewerAction {
    /// Display the FileUploadModal widget with the given file data.
    Show(FileData),
    /// Upload the file with the given data.
    Upload(FileData),
    /// Hide the FileUploadModal widget.
    Hide,
    /// No action.
    None,
}

/// Type alias for file data message sent through the channel.
pub type FileData = Arc<(FilePreviewerMetaData, Option<Thumbnail>)>;

/// Type alias for the receiver that gets file data.
pub type FileLoadReceiver = std::sync::mpsc::Receiver<Option<FileData>>;

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
    fn set_content(&mut self, cx: &mut Cx, file_load_message: FileData) {
        self.file_data = Some(file_load_message.clone());
        self.set_metadata(cx, &file_load_message.clone());
        if let Some(thumbnail) = &file_load_message.1 {
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
        let close_button = self.view.button(ids!(close_button));
        let cancel_button = self.view.button(ids!(buttons_view.cancel_button));

        // Handle closing the modal via close button or cancel button
        let close_clicked = close_button.clicked(actions);
        let cancel_clicked = cancel_button.clicked(actions);
        if close_clicked || cancel_clicked ||
            actions.iter().any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            // If the modal was dismissed by clicking outside of it, we MUST NOT emit
            // a FilePreviewerAction::Hide action, as that would cause
            // an infinite action feedback loop.
            if close_clicked || cancel_clicked {
                cx.action(FilePreviewerAction::Hide);
            }
            self.file_data = None;
            return;
        }

        if self.view.button(ids!(buttons_view.upload_button)).clicked(actions) {
            if let Some(file_data) = self.file_data.take() {
                cx.action(FilePreviewerAction::Upload(file_data));
            }
            return;
        }

        for action in actions {
            if let Some(FilePreviewerAction::Show(file_data)) = action.downcast_ref() {
                self.set_content(cx, file_data.clone());
                self.file_data = Some(file_data.clone());
                continue;
            }
        }
    }
}

impl FileUploadModal {
    /// Sets the displayed file metadata (filename and formatted size).
    pub fn set_metadata(&mut self, cx: &mut Cx, file_data: &FileData) {
        let formatted_size = format_file_size(file_data.0.file_size);
        let display_text = format!("{} - ({})", file_data.0.filename, formatted_size);
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

/// Converts bytes to a human-readable file size string (e.g., "1.5 MB", "512 KB").
///
/// Uses binary units (1024 bytes = 1 KB) for conversion.
/// For sizes less than 1 KB, displays the exact byte count without decimal places.
pub fn format_file_size(bytes: u64) -> String {
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
        // Show exact bytes without decimal for values < 1 KB
        format!("{} B", bytes)
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Metadata for a file to be previewed.
#[derive(Debug, Clone)]
pub struct FilePreviewerMetaData {
    pub filename: String,
    pub mime: mime_guess::Mime,
    /// File size in bytes
    pub file_size: u64,
    /// Path to the original file
    pub file_path: std::path::PathBuf,
}
