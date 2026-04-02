//! A modal dialog for previewing and confirming file uploads.
//!
//! This modal shows a preview of the file (image thumbnail or file icon)
//! along with file metadata and upload/cancel buttons.

use makepad_widgets::*;
use std::path::PathBuf;

use crate::utils::format_file_size;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.FileUploadModal = set_type_default() do #(FileUploadModal::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: 400,
        height: Fit,
        flow: Down,
        padding: 20,
        spacing: 15,

        show_bg: true,
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 8.0
            shadow_color: #00000044
            shadow_radius: 10.0
            shadow_offset: vec2(0.0, 2.0)
        }

        // Header
        header := View {
            width: Fill,
            height: Fit,
            flow: Right,
            align: Align{x: 0.0, y: 0.5},
            spacing: 10,

            title := Label {
                width: Fill,
                draw_text +: {
                    text_style: TITLE_TEXT { font_size: 14 },
                    color: (COLOR_TEXT)
                }
                text: "Upload File"
            }

            close_button := RobrixNeutralIconButton {
                width: Fit,
                height: Fit,
                align: Align{x: 1.0, y: 0.0},
                spacing: 0,
                margin: Inset{top: 4.5} // vertically align with the title
                padding: 15,
                draw_icon.svg: (ICON_CLOSE)
                icon_walk: Walk{width: 14, height: 14}
            }
        }

        // Preview area
        preview_container := View {
            width: Fill,
            height: 200,
            flow: Overlay,
            align: Align{x: 0.5, y: 0.5},

            show_bg: true,
            draw_bg.color: (COLOR_SECONDARY)

            // Image preview container (visible when file is an image)
            image_preview_container := View {
                visible: false,
                width: Fill,
                height: Fill,
                align: Align{x: 0.5, y: 0.5},
                // cannot center align for tall images
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

            file_name_label := Label {
                width: Fill,
                flow: Flow.Right{wrap: true},
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 11 },
                    color: (COLOR_TEXT)
                }
                text: ""
            }

            file_size_label := Label {
                width: Fill,
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10 },
                    color: (SMALL_STATE_TEXT_COLOR)
                }
                text: ""
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
                padding: Inset{top: 8, bottom: 8, left: 16, right: 16}
                text: "Cancel"
            }

            upload_button := RobrixPositiveIconButton {
                padding: Inset{top: 8, bottom: 8, left: 16, right: 16}
                draw_icon +: { svg: (ICON_UPLOAD) }
                icon_walk: Walk{width: 16, height: Fit, margin: Inset{right: 4}}
                text: "Upload"
            }
        }
    }
}

/// Data describing a file to be uploaded.
#[derive(Clone, Debug)]
pub struct FileData {
    /// The file path on the local filesystem.
    pub path: PathBuf,
    /// The file name (without directory path).
    pub name: String,
    /// The MIME type of the file.
    pub mime_type: String,
    /// The raw file data.
    pub data: Vec<u8>,
    /// The file size in bytes.
    pub size: u64,
    /// Optional thumbnail data for images (JPEG bytes).
    pub thumbnail: Option<ThumbnailData>,
}

/// Thumbnail data for image files.
#[derive(Clone, Debug)]
pub struct ThumbnailData {
    /// The thumbnail image data (JPEG).
    pub data: Vec<u8>,
    /// Width of the thumbnail.
    pub width: u32,
    /// Height of the thumbnail.
    pub height: u32,
}

/// Metadata for the file previewer (used in background loading).
#[derive(Debug, Clone)]
pub struct FilePreviewerMetaData {
    /// MIME type of the file.
    pub mime: mime_guess::Mime,
    /// File size in bytes.
    pub file_size: u64,
    /// Path to the original file.
    pub file_path: PathBuf,
}

/// Data loaded from a file by a background thread.
/// This is sent through a channel and combined with additional data to create `FileData`.
#[derive(Debug, Clone)]
pub struct FileLoadedData {
    /// Metadata about the file (path, size, MIME type).
    pub metadata: FilePreviewerMetaData,
    /// Optional thumbnail for image files.
    pub thumbnail: Option<ThumbnailData>,
    /// Optional dimensions for image/video files, width and height in pixels.
    pub dimensions: Option<(u32, u32)>,
}

/// Type alias for the receiver that gets loaded file data from a background thread.
pub type FileLoadReceiver = std::sync::mpsc::Receiver<Option<FileLoadedData>>;

/// Actions emitted by the FileUploadModal.
#[derive(Clone, Debug, Default)]
pub enum FilePreviewerAction {
    /// No action.
    #[default]
    None,
    /// Show the file upload modal with the given file data.
    Show(FileData),
    /// Hide the file upload modal.
    Hide,
    /// User confirmed the upload.
    UploadConfirmed(FileData),
    /// User cancelled the upload.
    Cancelled,
}

/// A modal for previewing and confirming file uploads.
#[derive(Script, ScriptHook, Widget)]
pub struct FileUploadModal {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// The current file data being previewed.
    #[rust] file_data: Option<FileData>,
}

impl Widget for FileUploadModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            // Handle close button
            if self.button(cx, ids!(close_button)).clicked(actions)
                || self.button(cx, ids!(cancel_button)).clicked(actions)
            {
                Cx::post_action(FilePreviewerAction::Cancelled);
                Cx::post_action(FilePreviewerAction::Hide);
            }

            // Handle upload button
            if self.button(cx, ids!(upload_button)).clicked(actions) {
                if let Some(file_data) = self.file_data.take() {
                    Cx::post_action(FilePreviewerAction::UploadConfirmed(file_data));
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

impl FileUploadModal {
    /// Sets the file data and updates the preview UI.
    pub fn set_file_data(&mut self, cx: &mut Cx, file_data: FileData) {
        // Update file name label
        self.label(cx, ids!(file_name_label))
            .set_text(cx, &file_data.name);

        // Update file size label
        self.label(cx, ids!(file_size_label))
            .set_text(cx, &format_file_size(file_data.size));

        // Determine if this is an image
        let is_image = crate::image_utils::is_displayable_image(&file_data.mime_type);

        // Show/hide appropriate preview widgets
        let image_preview = self.view.image(cx, ids!(image_preview_container.image_preview));
        let image_preview_container = self.view.view(cx, ids!(image_preview_container));
        let file_icon_container = self.view.view(cx, ids!(file_icon_container));

        if is_image {
            makepad_widgets::log!("FileUploadModal: Loading image preview, data size: {} bytes, mime: {}", file_data.data.len(), file_data.mime_type);
            // Hide file icon first
            file_icon_container.set_visible(cx, false);

            // Load image data into the preview
            if let Err(e) = crate::utils::load_png_or_jpg(&image_preview, cx, &file_data.data) {
                makepad_widgets::error!("Failed to load image preview: {:?}", e);
                // Fall back to file icon
                image_preview_container.set_visible(cx, false);
                file_icon_container.set_visible(cx, true);
                self.update_file_type_label(cx, &file_data.mime_type);
            } else {
                makepad_widgets::log!("FileUploadModal: Image loaded successfully");
                // Set container visible after loading
                image_preview_container.set_visible(cx, true);
            }
        } else {
            image_preview_container.set_visible(cx, false);
            file_icon_container.set_visible(cx, true);
            self.update_file_type_label(cx, &file_data.mime_type);
        }

        self.file_data = Some(file_data);
        self.redraw(cx);
    }

    /// Updates the file type label based on MIME type.
    fn update_file_type_label(&mut self, cx: &mut Cx, mime_type: &str) {
        let type_text = mime_type
            .split('/')
            .next_back()
            .unwrap_or("Unknown")
            .to_uppercase();
        self.label(cx, ids!(file_type_label))
            .set_text(cx, &format!("{} File", type_text));
    }
}

impl FileUploadModalRef {
    /// Sets the file data and updates the preview UI.
    pub fn set_file_data(&self, cx: &mut Cx, file_data: FileData) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_file_data(cx, file_data);
        }
    }
}
