//! A modal dialog for previewing and confirming file uploads.
//!
//! This modal shows a preview of the file (image preview or file icon)
//! along with file metadata and upload/cancel buttons.

use makepad_widgets::*;
use ruma::OwnedEventId;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::path::PathBuf;

use crate::{
    sliding_sync::{MatrixRequest, TimelineKind, submit_async_request},
    utils::format_decimal_file_size,
};
#[cfg(feature = "tsp")]
use crate::shared::popup_list::{PopupKind, enqueue_popup_notification};

/// File size above which the upload confirmation modal shows a warning.
pub const LARGE_ATTACHMENT_WARNING_THRESHOLD_BYTES: u64 = 10 * 1000 * 1000;

/// Unique identifier for a single file-upload attempt.
pub type FileUploadAttemptId = u64;

fn next_file_upload_attempt_id() -> FileUploadAttemptId {
    static NEXT_FILE_UPLOAD_ATTEMPT_ID: AtomicU64 = AtomicU64::new(1);
    NEXT_FILE_UPLOAD_ATTEMPT_ID.fetch_add(1, Ordering::Relaxed)
}

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.FileUploadModal = set_type_default() do #(FileUploadModal::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill { max: 1000 }
        height: Fill
        margin: 40,
        align: Align{x: 0.5, y: 0}
        flow: Down
        padding: Inset{top: 20, right: 25, bottom: 20, left: 25}

        scroll_bars: ScrollBars {
            show_scroll_x: false, show_scroll_y: true,
            scroll_bar_y: ScrollBar {drag_scrolling: true}
        }

        show_bg: true,
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 8.0
            border_size: 0.0
        }

        header := View {
            width: Fill,
            height: Fit,
            flow: Right,
            align: Align{x: 0.0, y: 0.5},
            spacing: 10,

            title := Label {
                width: Fill,
                padding: 0,
                margin: 0
                draw_text +: {
                    text_style: TITLE_TEXT { font_size: 14 },
                    color: (COLOR_TEXT)
                }
                text: "Upload File"
            }
        }

        // Preview area - fills available space with image/icon centered
        preview_container := View {
            width: Fill,
            height: Fill,
            flow: Overlay,
            align: Align{x: 0.5, y: 0.5},

            show_bg: true,
            draw_bg.color: (COLOR_SECONDARY)

            // Image preview container (visible when file is an image)
            image_preview_container := View {
                visible: false,
                width: Fill,
                height: Fill,
                image_preview := Image {
                    width: Fill,
                    height: Fill,
                    fit: ImageFit.Smallest,
                }
            }

            // File icon (visible when file is not an image)
            file_icon_container := View {
                visible: false,
                width: Fill,
                height: Fill,
                align: Align{x: 0.5, y: 0.5},
                flow: Down,
                spacing: 10,

                Icon {
                    width: Fit, height: Fit,
                    draw_icon +: {
                        svg: (ICON_FILE)
                        color: (COLOR_TEXT)
                    }
                    icon_walk: Walk{width: 64, height: 64}
                }

                file_type_label := Label {
                    width: Fit,
                    padding: 0,
                    margin: 0
                    draw_text +: {
                        text_style: REGULAR_TEXT { font_size: 10 },
                        color: (SMALL_STATE_TEXT_COLOR)
                    }
                    text: ""
                }
            }
        }

        // File info
        file_info := View {
            width: Fill,
            height: Fit,
            flow: Down,
            spacing: 5,

            caption_input := RobrixTextInput {
                width: Fill,
                height: Fit,
                is_multiline: true,
                empty_text: "Caption"
                padding: 10,
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 11 },
                    color: (COLOR_TEXT),
                }
            }

            file_size_label := Label {
                width: Fill,
                padding: 0,
                margin: 0
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10 },
                    color: (SMALL_STATE_TEXT_COLOR)
                }
                text: ""
            }

            large_attachment_warning_label := Label {
                visible: false,
                width: Fill,
                padding: 0,
                margin: 0
                flow: Flow.Right { wrap: true }
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10 },
                    color: (COLOR_TEXT_WARNING_NOT_FOUND)
                }
                text: "This file is large (over 10 MB). Are you sure you want to upload it to the homeserver?"
            }
        }

        // Buttons
        buttons := View {
            width: Fill,
            height: Fit,
            flow: Right,
            align: Align{x: 1.0, y: 0.5},
            spacing: 10,

            cancel_button := RobrixNeutralIconButton {
                padding: 13
                text: "Cancel"
            }

            upload_button := RobrixPositiveIconButton {
                padding: 13
                draw_icon +: { svg: (ICON_UPLOAD) }
                icon_walk: Walk{width: 16, height: Fit, margin: Inset{right: 4}}
                text: "Upload"
            }
        }
    }
}

/// Metadata describing a file to be uploaded.
#[derive(Clone, Debug)]
pub struct FileUploadMetadata {
    /// The file path on the local filesystem.
    pub path: PathBuf,
    /// The optional user-editable caption to send with the attachment.
    pub caption: Option<String>,
    /// The MIME type of the file.
    pub mime_type: String,
    /// Optional preview data, loaded for displayable images.
    pub preview_data: Option<Arc<Vec<u8>>>,
    /// The file size in bytes.
    pub size: u64,
}

impl FileUploadMetadata {
    /// Returns the file name portion of the local path, or a fallback for invalid paths.
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Unknown file")
            .to_string()
    }
}

/// Metadata for a pending attachment upload.
#[derive(Clone, Debug)]
pub struct AttachmentUpload {
    /// The timeline that was active when the file picker was opened.
    pub timeline_kind: TimelineKind,
    /// The selected file and preview data.
    pub file_data: FileUploadMetadata,
    /// The explicit event being replied to, if any.
    pub in_reply_to: Option<OwnedEventId>,
    /// Whether TSP signing was enabled when the file picker was opened.
    #[cfg(feature = "tsp")]
    pub sign_with_tsp: bool,
}

/// Actions used to show/hide the FileUploadModal.
#[derive(Clone, Debug, Default)]
pub enum FilePreviewerAction {
    /// No action.
    #[default]
    None,
    /// Request that the file upload modal be shown for the selected attachment.
    Show {
        upload: AttachmentUpload,
    },
    /// Report that the file upload modal should be hidden.
    Hide,
}

/// A modal for previewing and confirming file uploads.
#[derive(Script, ScriptHook, Widget)]
pub struct FileUploadModal {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// The attachment currently being previewed.
    #[rust] upload: Option<AttachmentUpload>,
}

impl Widget for FileUploadModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            // Handle cancel button
            if self.button(cx, ids!(cancel_button)).clicked(actions) {
                self.upload = None;
                Cx::post_action(FilePreviewerAction::Hide);
            }

            // Handle upload button
            if self.button(cx, ids!(upload_button)).clicked(actions) {
                let caption = match self.text_input(cx, ids!(caption_input)).text().trim() {
                    "" => None,
                    caption => Some(caption.to_string()),
                };
                if let Some(upload) = self.upload.as_mut() {
                    upload.file_data.caption = caption;
                    let mut upload = upload.clone();
                    upload.file_data.preview_data = None;
                    submit_attachment_upload(upload);
                    self.upload = None;
                    Cx::post_action(FilePreviewerAction::Hide);
                }
            }
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

/// Submits a confirmed attachment upload request to the Matrix worker.
pub fn submit_attachment_upload(upload: AttachmentUpload) {
    #[cfg(feature = "tsp")]
    if upload.sign_with_tsp {
        enqueue_popup_notification(
            "TSP-signed attachment uploads are not supported yet. Disable TSP signing to upload files.",
            PopupKind::Error,
            None,
        );
        return;
    }

    let upload_id = next_file_upload_attempt_id();
    submit_async_request(MatrixRequest::SendAttachment {
        upload_id,
        upload,
    });
}

impl FileUploadModal {
    /// Sets the selected attachment upload and updates the preview UI.
    pub fn set_upload(&mut self, cx: &mut Cx, upload: AttachmentUpload) {
        let file_data = &upload.file_data;
        let file_name = file_data.file_name();
        let caption = file_data.caption.as_deref().unwrap_or(&file_name);
        self.button(cx, ids!(cancel_button)).reset_hover(cx);
        self.button(cx, ids!(upload_button)).reset_hover(cx);
        self.text_input(cx, ids!(caption_input)).set_text(cx, caption);
        self.label(cx, ids!(file_size_label))
            .set_text(cx, &format_decimal_file_size(file_data.size));
        self.label(cx, ids!(large_attachment_warning_label))
            .set_visible(cx, file_data.size > LARGE_ATTACHMENT_WARNING_THRESHOLD_BYTES);

        // Show image preview if this is a displayable image
        let is_image = crate::image_utils::is_displayable_image(&file_data.mime_type);
        let image_preview = self.view.image(cx, ids!(image_preview_container.image_preview));
        let image_preview_container = self.view.view(cx, ids!(image_preview_container));
        let file_icon_container = self.view.view(cx, ids!(file_icon_container));
        if is_image {
            // Hide file icon first
            file_icon_container.set_visible(cx, false);

            // Load image data into the preview
            if let Some(preview_data) = &file_data.preview_data {
                if let Err(e) = crate::utils::load_png_or_jpg(&image_preview, cx, preview_data) {
                    makepad_widgets::error!("Failed to load image preview: {:?}", e);
                    // Fall back to file icon
                    image_preview_container.set_visible(cx, false);
                    file_icon_container.set_visible(cx, true);
                    self.update_file_type_label(cx, &file_data.mime_type);
                } else {
                    // Set container visible after loading
                    image_preview_container.set_visible(cx, true);
                }
            } else {
                image_preview_container.set_visible(cx, false);
                file_icon_container.set_visible(cx, true);
                self.update_file_type_label(cx, &file_data.mime_type);
            }
        } else {
            image_preview_container.set_visible(cx, false);
            file_icon_container.set_visible(cx, true);
            self.update_file_type_label(cx, &file_data.mime_type);
        }

        self.upload = Some(upload);
        self.redraw(cx);
    }

    /// Updates the file type label based on MIME type.
    fn update_file_type_label(&mut self, cx: &mut Cx, mime_type: &str) {
        self.label(cx, ids!(file_type_label))
            .set_text(cx, display_file_type_label(mime_type));
    }
}

fn display_file_type_label(mime_type: &str) -> &'static str {
    let mime_type = mime_type
        .split(';')
        .next()
        .unwrap_or(mime_type)
        .trim()
        .to_ascii_lowercase();

    match mime_type.as_str() {
        "text/plain" => "Plain text file",
        "text/markdown" | "text/x-markdown" => "Markdown file",
        "text/csv" => "CSV spreadsheet",
        "text/html" => "HTML document",
        "text/css" => "CSS stylesheet",
        "text/javascript" | "application/javascript" | "application/x-javascript" => "JavaScript file",
        "text/xml" | "application/xml" => "XML document",
        "application/json" => "JSON file",
        "application/pdf" => "PDF document",
        "application/rtf" | "text/rtf" => "Rich text document",
        "application/msword" |
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "Word document",
        "application/vnd.ms-excel" |
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "Excel spreadsheet",
        "application/vnd.ms-powerpoint" |
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "PowerPoint presentation",
        "application/zip" => "ZIP archive",
        "application/x-tar" => "TAR archive",
        "application/gzip" | "application/x-gzip" => "Gzip archive",
        "application/x-bzip2" => "Bzip2 archive",
        "application/x-7z-compressed" => "7-Zip archive",
        "application/vnd.rar" | "application/x-rar-compressed" => "RAR archive",
        "application/x-sh" => "Shell script",
        "application/x-sql" => "SQL file",
        "image/png" => "PNG image",
        "image/jpeg" | "image/jpg" => "JPEG image",
        "image/gif" => "GIF image",
        "image/webp" => "WebP image",
        "image/bmp" => "BMP image",
        "image/svg+xml" => "SVG image",
        "image/tiff" => "TIFF image",
        "audio/mpeg" => "MP3 audio",
        "audio/mp4" => "MPEG-4 audio",
        "audio/wav" | "audio/x-wav" => "WAV audio",
        "audio/ogg" => "Ogg audio",
        "audio/flac" => "FLAC audio",
        "video/mp4" => "MP4 video",
        "video/webm" => "WebM video",
        "video/quicktime" => "QuickTime video",
        "video/x-msvideo" => "AVI video",
        "font/ttf" | "font/otf" | "font/woff" | "font/woff2" => "Font file",
        _ if mime_type.starts_with("text/") => "Text file",
        _ if mime_type.starts_with("image/") => "Image file",
        _ if mime_type.starts_with("audio/") => "Audio file",
        _ if mime_type.starts_with("video/") => "Video file",
        _ if mime_type.starts_with("font/") => "Font file",
        _ => "File",
    }
}

impl FileUploadModalRef {
    /// Sets the selected attachment upload and updates the preview UI.
    pub fn set_upload(&self, cx: &mut Cx, upload: AttachmentUpload) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_upload(cx, upload);
        }
    }
}
