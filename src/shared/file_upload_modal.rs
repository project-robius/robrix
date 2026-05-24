//! A modal dialog for previewing and confirming file uploads.
//!
//! This modal shows a preview of the file (image thumbnail or file icon)
//! along with file metadata and upload/cancel buttons.

use bytesize::ByteSize;
use makepad_widgets::*;
use ruma::OwnedEventId;
use std::path::PathBuf;
use std::sync::Arc;

use crate::home::room_screen::TimelineUpdate;

/// Type alias for the sender used to send timeline updates.
pub type TimelineUpdateSender = crossbeam_channel::Sender<TimelineUpdate>;

/// File size above which the upload confirmation modal shows a warning.
pub const LARGE_ATTACHMENT_WARNING_THRESHOLD_BYTES: u64 = 10 * 1024 * 1024;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.FileUploadModal = set_type_default() do #(FileUploadModal::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill { max: 1000 }
        // TODO: i'd like for this height to be Fit with a max of Rel { base: Full, factor: 0.90 },
        //       but Makepad doesn't allow Fit views with a max to be scrolled.
        height: Fill // { max: 1400 }
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
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10 },
                    color: (SMALL_STATE_TEXT_COLOR)
                }
                text: ""
            }

            large_attachment_warning_label := Label {
                visible: false,
                width: Fill,
                flow: Flow.Right { wrap: true }
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10 },
                    color: (COLOR_TEXT_WARNING_NOT_FOUND)
                }
                text: "Are you sure you want to upload this large file (over 10 MB) to the homeserver?"
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

/// Metadata describing a file to be uploaded.
#[derive(Clone, Debug)]
pub struct FileUploadMetadata {
    /// The file path on the local filesystem.
    pub path: PathBuf,
    /// The optional user-editable caption to send with the attachment.
    pub caption: Option<String>,
    /// The MIME type of the file.
    pub mime_type: String,
    /// Optional preview data, only loaded for reasonably small displayable images.
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
    /// The selected file and preview data.
    pub file_data: FileUploadMetadata,
    /// The explicit event being replied to, if any.
    pub in_reply_to: Option<OwnedEventId>,
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
    /// Optional preview data read from disk.
    pub preview_data: Option<Arc<Vec<u8>>>,
}

/// Actions used to show/hide the FileUploadModal and report user confirmation.
#[derive(Clone, Debug, Default)]
pub enum FilePreviewerAction {
    /// No action.
    #[default]
    None,
    /// Request that the file upload modal be shown with the given file data and timeline update sender.
    /// The sender is captured at file selection time to ensure uploads go to the correct room.
    Show {
        file_data: FileUploadMetadata,
        timeline_update_sender: TimelineUpdateSender,
        in_reply_to: Option<OwnedEventId>,
    },
    /// Report that the file upload modal should be hidden.
    Hide,
}

/// A modal for previewing and confirming file uploads.
#[derive(Script, ScriptHook, Widget)]
pub struct FileUploadModal {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// The current file data being previewed.
    #[rust] file_data: Option<FileUploadMetadata>,
    /// The sender to use for sending timeline updates when upload is confirmed.
    #[rust] timeline_update_sender: Option<TimelineUpdateSender>,
    /// The reply event captured when the file picker was opened.
    #[rust] in_reply_to: Option<OwnedEventId>,
}

impl Widget for FileUploadModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            // Handle cancel button
            if self.button(cx, ids!(cancel_button)).clicked(actions) {
                self.file_data = None;
                self.timeline_update_sender = None;
                self.in_reply_to = None;
                Cx::post_action(FilePreviewerAction::Hide);
            }

            // Handle upload button
            if self.button(cx, ids!(upload_button)).clicked(actions) {
                let caption = match self.text_input(cx, ids!(caption_input)).text().trim() {
                    "" => None,
                    caption => Some(caption.to_string()),
                };
                if let (Some(file_data), Some(sender)) = (self.file_data.as_mut(), self.timeline_update_sender.as_ref()) {
                    file_data.caption = caption;
                    let mut upload_file_data = file_data.clone();
                    upload_file_data.preview_data = None;
                    let upload = AttachmentUpload {
                        file_data: upload_file_data,
                        in_reply_to: self.in_reply_to.clone(),
                    };
                    match sender.send(TimelineUpdate::FileUploadConfirmed(upload)) {
                        Ok(()) => {
                            self.file_data = None;
                            self.timeline_update_sender = None;
                            self.in_reply_to = None;
                            SignalToUI::set_ui_signal();
                            Cx::post_action(FilePreviewerAction::Hide);
                        }
                        Err(_e) => {
                            crate::shared::popup_list::enqueue_popup_notification(
                                "Cannot upload file: the selected room is no longer available.",
                                crate::shared::popup_list::PopupKind::Error,
                                None,
                            );
                        }
                    }
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
    /// Sets the file data and timeline update sender, and updates the preview UI.
    pub fn set_file_data(&mut self, cx: &mut Cx, file_data: FileUploadMetadata, timeline_update_sender: TimelineUpdateSender, in_reply_to: Option<OwnedEventId>) {
        let file_name = file_data.file_name();
        let caption = file_data.caption.as_deref().unwrap_or(&file_name);
        self.text_input(cx, ids!(caption_input)).set_text(cx, caption);
        self.label(cx, ids!(file_size_label))
            .set_text(cx, &ByteSize::b(file_data.size).to_string());
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

        self.file_data = Some(file_data);
        self.timeline_update_sender = Some(timeline_update_sender);
        self.in_reply_to = in_reply_to;
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
    /// Sets the file data and timeline update sender, and updates the preview UI.
    pub fn set_file_data(&self, cx: &mut Cx, file_data: FileUploadMetadata, timeline_update_sender: TimelineUpdateSender, in_reply_to: Option<OwnedEventId>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_file_data(cx, file_data, timeline_update_sender, in_reply_to);
        }
    }
}
