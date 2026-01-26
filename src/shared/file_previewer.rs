//! A file previewer widget that displays file metadata and previews.
//!
//! This widget shows:
//! - File metadata: filename and file size (always displayed)
//! - For images: displays the image preview
//! - For documents: displays only the filename
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! // Set the file metadata
//! file_previewer.set_metadata(cx, "document.pdf", 1024 * 500); // 500 KB file
//!
//! // For image files, show the image preview
//! file_previewer.show_image(cx, |cx, image_ref| {
//!     // Load and set the image texture
//!     image_ref.load_from_path(cx, "path/to/image.jpg")?;
//!     Ok(())
//! })?;
//!
//! // For document files, show the document icon
//! file_previewer.show_document(cx);
//! ```

use makepad_widgets::*;
use makepad_widgets::image_cache::{ImageBuffer, ImageError};
use mime_guess::{Mime, mime};
use std::path::PathBuf;

/// Loads the given image `data` into an `ImageBuffer` as either a PNG or JPEG.
fn load_image_from_bytes(data: &[u8]) -> Result<ImageBuffer, ImageError> {
    match imghdr::from_bytes(data) {
        Some(imghdr::Type::Png) => {
            ImageBuffer::from_png(data)
        },
        Some(imghdr::Type::Jpeg) => {
            ImageBuffer::from_jpg(data)
        },
        Some(_unsupported) => {
            Err(ImageError::UnsupportedFormat)
        }
        None => {
            Err(ImageError::UnsupportedFormat)
        }
    }
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    FilePreviewerButton = <RobrixIconButton> {
        width: 44, height: 44
        align: {x: 0.5, y: 0.5},
        spacing: 0,
        padding: 0,
        draw_bg: {
            color: (COLOR_SECONDARY * 0.925)
        }
        draw_icon: {
            svg_file: (ICON_ZOOM_OUT),
            fn get_color(self) -> vec4 {
                return #x0;
            }
        }
        icon_walk: {width: 27, height: 27}
    }

    pub FilePreviewer = {{FilePreviewer}} {
        width: Fit, height: Fit
        wrapper = <RoundedView> {
            width: Fit, height: Fit
            align: {x: 0.5}
            flow: Down
            padding: {top: 30, right: 40, bottom: 20, left: 40}

            show_bg: true
            draw_bg: {
                color: (COLOR_PRIMARY)
                border_radius: 4
            }

            header_view = <View> {
                width: 400
                height: Fit
                flow: Right
                align: {x: 1.0, y: 0.0}
                margin: {top: -15, right: -25, bottom: 10}

                close_button = <FilePreviewerButton> {
                    draw_icon: { svg_file: (ICON_CLOSE) }
                    icon_walk: {width: 21, height: 21 }
                }
            }
            <View> {
                width: Fit, height: Fit
                flow: Right
                // Document view (visible only for non-image files)
                document_view = <View> {
                    visible: false,
                    width: Fit
                    height: 40
                    flow: Down
                    align: {x: 0.5, y: 0.5}
                    file_icon = <Icon> {
                        draw_icon: {
                            svg_file: (ICON_FILE),
                            color: #999,
                        }
                        icon_walk: { width: 20, height: 20 }
                    }
                }
                // File metadata section (always visible)
                metadata_view = <View> {
                    width: 400
                    height: Fit
                    flow: Down
                    spacing: 5

                    filename_text = <Label> {
                        width: Fill
                        height: Fit
                        draw_text: {
                            text_style: <REGULAR_TEXT>{font_size: 14},
                            color: #000,
                            wrap: Word
                        }
                    }
                }
            }

            // Image preview (visible only for image files)
            image_view = <View> {
                visible: true,
                width: 300
                height: 300
                flow: Down
                align: {x: 0.5, y: 0.5}

                preview_image = <Image> {
                    width: Fill, height: Fill,
                    fit: Stretch,
                }
            }

            buttons_view = <View> {
                width: 300, height: Fit
                flow: Right,
                margin: {right: -15}
                align: {x: 1.0, y: 0.5}
                spacing: 20

                cancel_button = <RobrixIconButton> {
                    width: 120,
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
                    width: 120
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
}

/// Actions emitted by the `FilePreviewer` widget.
#[derive(Clone, Debug, DefaultNone)]
pub enum FilePreviewerAction {
    /// No action.
    None,
    /// Display the FilePreviewer widget with the given file path and room context.
    Show {
        file_path: PathBuf,
    },
    /// Upload the file with the given path.
    Upload {
        file_path: PathBuf,
    },
    /// Hide the FilePreviewer widget.
    Hide,
}

/// Type alias for file data message sent through the channel.
type FileLoadMessage = (FilePreviewerMetaData, Vec<u8>);

/// Type alias for the receiver that gets file data.
type FileLoadReceiver = std::sync::mpsc::Receiver<FileLoadMessage>;

/// A widget that previews files by displaying metadata and content based on file type.
#[derive(Live, Widget, LiveHook)]
pub struct FilePreviewer {
    #[redraw] #[deref] view: View,
    #[rust] file_type: FileType,
    #[rust] background_task_id: u32,
    #[rust] receiver: Option<(u32, FileLoadReceiver)>,
    #[rust] file_path: Option<PathBuf>,
}

impl Widget for FilePreviewer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
        if let (Event::Signal, Some((_background_task_id, receiver))) = (event, &mut self.receiver) {
            let mut remove_receiver = false;
            match receiver.try_recv() {
                Ok((file_meta, file)) => {
                    // Set the metadata (filename and file size)
                    self.set_metadata(cx, &file_meta.filename, file_meta.file_size as u64);
                    self.view.button(ids!(close_button)).reset_hover(cx);
                    self.view.button(ids!(close_button)).set_enabled(cx, true);
                    if file_meta.mime.type_() == mime::IMAGE {
                        // Try to load the image from the file data
                        if let Ok(image_buffer) = load_image_from_bytes(&file) {
                            let image_ref = self.view.image(ids!(wrapper.image_view.preview_image));
                            let texture = image_buffer.into_new_texture(cx);

                            // Set the texture
                            image_ref.set_texture(cx, Some(texture));

                            // Show image view, hide document view
                            self.view(ids!(wrapper.image_view)).set_visible(cx, true);
                            self.view(ids!(wrapper.document_view)).set_visible(cx, false);
                            self.file_type = FileType::Image;

                        } else {
                            println!("Failed to load image from bytes");
                            // Failed to load image, show as document
                            self.show_document(cx);
                        }
                    } else {
                        // Not an image, show document view
                        self.show_document(cx);
                    }
                    remove_receiver = true;
                    // Redraw to display the image
                    self.redraw(cx);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    remove_receiver = true;
                }
            }
            if remove_receiver {
                self.receiver = None;
            }
        }
        
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for FilePreviewer {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        // Handle close button click
        if self.view.button(ids!(wrapper.header_view.close_button)).clicked(actions) {
            cx.action(FilePreviewerAction::Hide);
            return;
        }
        //close_modal_button
        if self.view.button(ids!(close_modal_button)).clicked(actions) {
            cx.action(FilePreviewerAction::Hide);
            return;
        }
        // Handle cancel button click
        if self.view.button(ids!(wrapper.buttons_view.cancel_button)).clicked(actions) {
            cx.action(FilePreviewerAction::Hide);
            self.view.button(ids!(wrapper.buttons_view.cancel_button)).set_enabled(cx, false);
            return;
        }
        // Handle upload button click
        if self.view.button(ids!(wrapper.buttons_view.upload_button)).clicked(actions) {
            if let Some(file_path) = &self.file_path {
                cx.action(FilePreviewerAction::Upload {
                    file_path: file_path.clone(),
                });
                cx.action(FilePreviewerAction::Hide);
                self.view.button(ids!(wrapper.buttons_view.upload_button)).set_enabled(cx, false);
            }
            return;
        }
        for action in actions {
            if let Some(FilePreviewerAction::Show { file_path }) = action.downcast_ref() {
                self.view.button(ids!(close_button)).reset_hover(cx);
                self.view.button(ids!(close_button)).set_enabled(cx, true);
                // Reset button states
                self.view.button(ids!(wrapper.buttons_view.cancel_button)).reset_hover(cx);
                self.view.button(ids!(wrapper.buttons_view.cancel_button)).set_enabled(cx, true);
                self.view.button(ids!(wrapper.buttons_view.upload_button)).reset_hover(cx);
                self.view.button(ids!(wrapper.buttons_view.upload_button)).set_enabled(cx, true);
                // Store the context for later use when upload button is clicked
                self.file_path = Some(file_path.clone());
                let (sender, receiver) = std::sync::mpsc::channel();
                if let Some(new_value) = self.background_task_id.checked_add(1) {
                    self.background_task_id = new_value;
                }
                self.receiver = Some((self.background_task_id, receiver));
                let file_path_clone = file_path.clone();
                let filename = file_path_clone
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                // Detect the mime type
                let mime_str = mime_guess::from_path(&file_path_clone)
                    .first_or_octet_stream()
                    .to_string();
                use mime_guess::mime;
                let mime: mime::Mime = mime_str.parse().unwrap_or(mime::APPLICATION_OCTET_STREAM);

                cx.spawn_thread(move || {
                    if let Ok(file) = std::fs::read(file_path_clone) {
                        let metadata = FilePreviewerMetaData {
                            filename,
                            mime,
                            file_size: file.len(),
                        };
                        let _ = sender.send((metadata, file));
                        SignalToUI::set_ui_signal();
                    }
                });
                continue;
            }
        }
    }
}

impl FilePreviewer {
    /// Sets the file metadata (filename and size).
    ///
    /// ## Arguments
    /// * `filename`: the name of the file
    /// * `filesize`: the size of the file in bytes
    pub fn set_metadata(&mut self, cx: &mut Cx, filename: &str, filesize: u64) {
        let size_str = format_file_size(filesize);
        self.view.label(ids!(filename_text)).set_text(cx, &format!("{} - ({})", filename, size_str));
    }

    /// Displays an image preview.
    ///
    /// ## Arguments
    /// * `image_set_function`: a function that sets the image content on the provided ImageRef
    pub fn show_image<F, E>(&mut self, cx: &mut Cx, image_set_function: F) -> Result<(), E>
        where F: FnOnce(&mut Cx, ImageRef) -> Result<(), E>
    {
        let image_ref = self.view.image(ids!(wrapper.image_view.preview_image));
        match image_set_function(cx, image_ref) {
            Ok(_) => {
                self.file_type = FileType::Image;
                self.view(ids!(wrapper.image_view)).set_visible(cx, true);
                self.view(ids!(wrapper.document_view)).set_visible(cx, false);
                Ok(())
            }
            Err(e) => {
                self.show_document(cx);
                Err(e)
            }
        }
    }

    /// Displays the document view (just an icon, metadata already shown).
    pub fn show_document(&mut self, cx: &mut Cx) {
        self.file_type = FileType::Document;
        self.view(ids!(wrapper.document_view)).set_visible(cx, true);
        self.view(ids!(wrapper.image_view)).set_visible(cx, false);
    }

    /// Returns the current file type being displayed.
    pub fn file_type(&self) -> FileType {
        self.file_type
    }
}

impl FilePreviewerRef {
    /// See [FilePreviewer::set_metadata()].
    pub fn set_metadata(&self, cx: &mut Cx, filename: &str, filesize: u64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_metadata(cx, filename, filesize);
        }
    }

    /// See [FilePreviewer::show_image()].
    pub fn show_image<F, E>(&self, cx: &mut Cx, image_set_function: F) -> Result<(), E>
        where F: FnOnce(&mut Cx, ImageRef) -> Result<(), E>
    {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_image(cx, image_set_function)
        } else {
            Ok(())
        }
    }

    /// See [FilePreviewer::show_document()].
    pub fn show_document(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_document(cx);
        }
    }

    /// See [FilePreviewer::file_type()].
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

/// Convert bytes to human-readable file size format
fn format_file_size(bytes: u64) -> String {
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

#[derive(Debug, Clone)]
/// Metadata for a file.
pub struct FilePreviewerMetaData {
    pub filename: String,
    pub mime: Mime,
    // Image size in bytes
    pub file_size: usize,
}