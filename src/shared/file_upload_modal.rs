//! A modal dialog for previewing and confirming file uploads.
//!
//! Also includes various helper functions to uploading/previewing attachments.

use makepad_code_editor::code_view::CodeViewWidgetExt;
use makepad_widgets::*;
use makepad_widgets::image_cache::{ImageBuffer, decode_image_from_data};
use ruma::OwnedEventId;
use std::{io::Read, sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}}};
use crate::{
    sliding_sync::{MatrixRequest, TimelineKind, submit_async_request},
    utils::format_decimal_file_size,
};

const TEXT_PREVIEW_MAX_BYTES: u64 = 128 * 1024;

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

    mod.widgets.PreviewTruncatedLabel = Label {
        visible: false,
        width: Fill, height: Fit,
        padding: 0
        flow: Flow.Right { wrap: true }
        margin: Inset{top: 8}
        draw_text +: {
            text_style: REGULAR_TEXT { font_size: 10 },
            color: (SMALL_STATE_TEXT_COLOR)
        }
        text: "Preview truncated to 128 KB."
    }

    mod.widgets.FileUploadModal = set_type_default() do #(FileUploadModal::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill { max: 1000 }
        height: Fill
        margin: 30,
        align: Align{x: 0.5, y: 0}
        flow: Down
        spacing: 12
        padding: Inset{top: 20, right: 25, bottom: 20, left: 25}

        scroll_bars: ScrollBars {
            show_scroll_x: false, show_scroll_y: true,
            scroll_bar_y: ScrollBar {drag_scrolling: true}
        }

        show_bg: true,
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 6.0
            border_size: 0.0
        }

        header := View {
            width: Fill, height: Fit,
            flow: Right,
            align: Align{y: 0.5},
            spacing: 10,

            title := Label {
                width: Fill, height: Fit,
                draw_text +: {
                    text_style: TITLE_TEXT { font_size: 16 },
                    color: #000
                }
                text: "Upload File"
            }

            close_button := RobrixIconButton {
                width: Fit, height: Fit,
                padding: 12,
                spacing: 0
                align: Align{x: 0.5, y: 0.5}
                icon_walk: Walk{width: 18, height: 18, margin: 0}
                draw_icon.svg: (ICON_CLOSE)
                draw_icon.color: #666
                draw_bg +: {
                    border_size: 0
                    color: #0000
                    color_hover: #00000015
                    color_down: #00000025
                }
            }
        }

        caption_input := RobrixTextInput {
            width: Fill,
            height: Fit { max: FitBound.Rel{base: Base.Full, factor: 0.5} },
            is_multiline: true,
            submit_on_enter: true,
            empty_text: "Enter caption..."
            padding: 10,
            draw_text +: {
                text_style: REGULAR_TEXT { font_size: 11 },
                color: (COLOR_TEXT),
            }
        }

        file_info_label := Label {
            width: Fill, height: Fit,
            padding: 0
            margin: Inset { left: 5 }
            flow: Flow.Right { wrap: true }
            draw_text +: {
                text_style: REGULAR_TEXT { font_size: 11 },
                color: (SMALL_STATE_TEXT_COLOR)
            }
            text: ""
        }

        large_attachment_warning_label := Label {
            visible: false,
            width: Fill, height: Fit,
            padding: 0
            margin: Inset { left: 5 }
            flow: Flow.Right { wrap: true }
            draw_text +: {
                text_style: REGULAR_TEXT { font_size: 11 },
                color: (COLOR_TEXT_WARNING_NOT_FOUND)
            }
            text: "This file is large (over 10 MB). Are you sure you want to upload it to the homeserver?"
        }

        empty_attachment_warning_label := Label {
            visible: false,
            width: Fill, height: Fit,
            padding: 0
            margin: Inset { left: 5 }
            flow: Flow.Right { wrap: true }
            draw_text +: {
                text_style: REGULAR_TEXT { font_size: 11 },
                color: (COLOR_TEXT_WARNING_NOT_FOUND)
            }
            text: "This file is empty (0 bytes). Are you sure you want to upload it?"
        }

        // The view showing the preview of the file being uploaded, switching between
        // multiple options: loading page, image, code, plain text, or no preview.
        preview_block := View {
            width: Fill, height: Fill,
            flow: Overlay

            preview_flip := PageFlip {
                width: Fill, height: Fill,
                padding: 10
                active_page: @loading_page

                // Spinner shown while the preview loads.
                loading_page := View {
                    width: Fill, height: Fill,
                    flow: Right,
                    align: Align{x: 0.5, y: 0.5},
                    spacing: 12

                    LoadingSpinner {
                        width: 28, height: 28,
                        draw_bg +: { color: (SMALL_STATE_TEXT_COLOR) }
                    }
                    Label {
                        width: Fit, height: Fit,
                        draw_text +: {
                            text_style: REGULAR_TEXT { font_size: 13 },
                            color: (SMALL_STATE_TEXT_COLOR)
                        }
                        text: "Loading file preview..."
                    }
                }

                // Image preview, shown for displayable image files.
                image_page := View {
                    width: Fill, height: Fill,
                    flow: Down,
                    align: Align{x: 0.5, y: 0.5},
                    image_preview := Image {
                        width: Fill,
                        height: Fit { max: FitBound.Rel{base: Base.Full, factor: 0.99} },
                        fit: ImageFit.Smallest,
                    }
                }

                // Code preview, shown with syntax highlighting.
                code_text_page := View {
                    width: Fill, height: Fill,
                    flow: Down

                    code_preview := mod.widgets.LightCodeView {
                        editor +: {
                            width: Fill, height: Fill,
                            draw_text +: { text_style +: { font_size: 11 } }
                        }
                        text: ""
                    }
                    code_truncated_label := mod.widgets.PreviewTruncatedLabel {}
                }

                // Plaintext preview, shown without syntax highlighting.
                plain_text_page := View {
                    width: Fill, height: Fill,
                    flow: Down

                    plain_preview := mod.widgets.PlainCodeView {
                        editor +: {
                            width: Fill, height: Fill,
                            draw_text +: { text_style +: { font_size: 11 } }
                        }
                        text: ""
                    }
                    plain_truncated_label := mod.widgets.PreviewTruncatedLabel {}
                }

                // Note: add other types of previews here when they're supported.

                // Fallback shown when no preview can be generated.
                no_preview_page := View {
                    width: Fill, height: Fill,
                    flow: Down,
                    align: Align{x: 0.5, y: 0.5},
                    spacing: 12

                    no_preview_icon_badge := RoundedView {
                        width: 110, height: 110, align: Align{x: 0.5, y: 0.5}, show_bg: true,
                        draw_bg +: { pixel: fn() {
                            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                            let cx = self.rect_size.x * 0.5
                            let cy = self.rect_size.y * 0.5
                            let disc = 46.0
                            sdf.circle(cx, cy, disc)
                            sdf.fill(#D6E6FF)
                            let pw = 38.0
                            let ph = 48.0
                            let px = cx - pw * 0.5
                            let py = cy - ph * 0.5
                            let fold = 13.0
                            // Full rounded page; the fold is a triangle over the top-right corner.
                            sdf.box(px, py, pw, ph, 4.0)
                            sdf.fill(#FFFFFF)
                            sdf.move_to(px + pw - fold, py)
                            sdf.line_to(px + pw, py + fold)
                            sdf.line_to(px + pw - fold, py + fold)
                            sdf.close_path()
                            sdf.fill(#7AA8E8)
                            let lx = px + 7.0
                            let lw = pw - 14.0
                            sdf.box(lx, py + fold + 7.0, lw * 0.6, 3.0, 1.5)
                            sdf.fill(#7AA8E8)
                            sdf.box(lx, py + fold + 16.0, lw, 3.0, 1.5)
                            sdf.fill(#7AA8E8)
                            sdf.box(lx, py + fold + 25.0, lw, 3.0, 1.5)
                            sdf.fill(#7AA8E8)
                            return sdf.result
                        } }
                    }

                    Label {
                        width: Fill, height: Fit,
                        align: Align {x: 0.5}
                        flow: Flow.Right { wrap: true }
                        draw_text +: {
                            text_style: TITLE_TEXT { font_size: 15 },
                            color: (SMALL_STATE_TEXT_COLOR)
                        }
                        text: "No preview available"
                    }
                }
            }

            border_frame := RoundedView {
                width: Fill, height: Fill,
                show_bg: true,
                draw_bg +: {
                    color: (COLOR_TRANSPARENT)
                    border_radius: 6.0
                    border_size: 1.0,
                    border_color: (COLOR_SECONDARY_DARKER)
                }
            }
        }

        buttons := View {
            width: Fill, height: Fit,
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
    /// The local source file. For Android content selections this is a temp
    /// copy, auto-deleted once this metadata and all its clones drop.
    pub source: robius_file_picker::LocalFile,
    /// The optional user-editable caption to send with the attachment.
    pub caption: Option<String>,
    /// The MIME type of the file.
    pub mime_type: String,
    /// A preview of the file's content, generated when the file was selected.
    pub preview: FilePreview,
    /// The file size in bytes.
    pub size: u64,
}

/// A preview of a pending upload's content.
#[derive(Clone, Debug, Default)]
pub enum FilePreview {
    /// No preview could be generated for this file.
    #[default]
    None,
    /// The preview is still being built on a background thread.
    Loading,
    /// A fully decoded image, uploaded to a GPU texture by the UI thread when shown.
    Image(ImageBuffer),
    /// A bounded UTF-8 text excerpt for text-like files.
    Text(TextPreview),
}

/// A simple file preview wrapper for cheap cloning within makepad actions.
#[derive(Clone, Debug)]
pub struct PreviewPayload(Arc<Mutex<Option<FilePreview>>>);

impl PreviewPayload {
    pub fn new(preview: FilePreview) -> Self {
        Self(Arc::new(Mutex::new(Some(preview))))
    }

    pub fn take(&self) -> Option<FilePreview> {
        self.0.lock().ok().and_then(|mut guard| guard.take())
    }
}

#[derive(Clone, Debug)]
pub struct TextPreview {
    pub content: String,
    pub truncated: bool,
    pub is_code: bool,
}

impl FileUploadMetadata {
    /// The local filesystem path of the file to upload.
    pub fn path(&self) -> &std::path::Path {
        self.source.path()
    }

    /// Returns the file name portion of the local path, or a fallback for invalid paths.
    pub fn file_name(&self) -> String {
        self.path()
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
    /// The event being replied to, if any.
    pub in_reply_to: Option<OwnedEventId>,
    /// Whether TSP signing was enabled when the file picker was opened.
    #[cfg(feature = "tsp")]
    pub sign_with_tsp: bool,
}

/// Actions handled by the file upload modal.
#[derive(Clone, Debug, Default)]
#[allow(clippy::large_enum_variant)]
pub enum FileUploadModalAction {
    #[default]
    None,
    /// Show the file upload modal (in its initial loading state).
    Show {
        upload: AttachmentUpload,
        preview_id: FileUploadAttemptId,
    },
    /// The preview has been generated in the background and is ready to be shown.
    PreviewReady {
        preview_id: FileUploadAttemptId,
        preview: PreviewPayload,
    },
    Hide,
}

/// A modal for previewing and confirming file uploads.
#[derive(Script, ScriptHook, Widget)]
pub struct FileUploadModal {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    #[rust] upload: Option<AttachmentUpload>,

    /// The ID of the preview that is being generated by a bg thread.
    #[rust] preview_id: Option<FileUploadAttemptId>,
}

impl Widget for FileUploadModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            if self.button(cx, ids!(cancel_button)).clicked(actions)
                || self.button(cx, ids!(close_button)).clicked(actions)
            {
                self.reset(cx);
                Cx::post_action(FileUploadModalAction::Hide);
            } else if actions.iter().any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed))) {
                self.reset(cx);
            }

            // Handle upload button being clicked or Enter being pressed in the caption input.
            let caption_input = self.text_input(cx, ids!(caption_input));
            if self.button(cx, ids!(upload_button)).clicked(actions)
                || caption_input.returned(actions).is_some()
            {
                let caption = match caption_input.text().trim() {
                    "" => None,
                    caption => Some(caption.to_string()),
                };
                if let Some(mut upload) = self.upload.take() {
                    upload.file_data.caption = caption;
                    // The preview is only needed by this modal, not the bg matrix task.
                    upload.file_data.preview = FilePreview::None;
                    submit_attachment_upload(upload);
                    self.reset(cx);
                    Cx::post_action(FileUploadModalAction::Hide);
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

/// Generates the file info string, like `"name • size • type"`.
fn file_info_text(file_data: &FileUploadMetadata, is_text_preview: bool) -> String {
    format!("{}  •  {}  •  {}",
        file_data.file_name(),
        format_decimal_file_size(file_data.size),
        crate::utils::file_type_label(&file_data.mime_type, is_text_preview),
    )
}

impl FileUploadModal {
    /// Shows the modal content with the loading spinner at first.
    ///
    /// The real preview will be shown later once it has been generated
    /// and delivered from the background thread.
    fn set_upload(&mut self, cx: &mut Cx, upload: AttachmentUpload, preview_id: FileUploadAttemptId) {
        let file_data = &upload.file_data;
        
        let caption = file_data.caption.as_deref().unwrap_or("");
        self.text_input(cx, ids!(caption_input)).set_text(cx, caption);
        self.label(cx, ids!(file_info_label)).set_text(cx, &file_info_text(file_data, false));
        self.label(cx, ids!(large_attachment_warning_label))
            .set_visible(cx, file_data.size > LARGE_ATTACHMENT_WARNING_THRESHOLD_BYTES);
        self.label(cx, ids!(empty_attachment_warning_label))
            .set_visible(cx, file_data.size == 0);

        self.preview_id = Some(preview_id);
        self.show_preview(cx, FilePreview::Loading);
        self.upload = Some(upload);

        self.button(cx, ids!(cancel_button)).reset_hover(cx);
        self.button(cx, ids!(upload_button)).reset_hover(cx);
        self.button(cx, ids!(close_button)).reset_hover(cx);
        self.redraw(cx);
    }

    /// Displays the generated filed preview.
    fn set_preview(&mut self, cx: &mut Cx, preview_id: FileUploadAttemptId, preview: FilePreview) {
        if self.preview_id != Some(preview_id) {
            return;
        }
        let is_text = matches!(preview, FilePreview::Text(_));
        if let Some(info) = self.upload.as_ref().map(|u| file_info_text(&u.file_data, is_text)) {
            self.label(cx, ids!(file_info_label)).set_text(cx, &info);
        }
        self.show_preview(cx, preview);
        self.redraw(cx);
    }

    fn show_preview(&mut self, cx: &mut Cx, preview: FilePreview) {
        let page_flip = self.page_flip(cx, ids!(preview_flip));
        match preview {
            FilePreview::Loading => {
                page_flip.set_active_page(cx, id!(loading_page));
            }
            FilePreview::Image(buffer) => {
                page_flip.set_active_page(cx, id!(image_page));
                let texture = buffer.into_new_texture(cx);
                self.image(cx, ids!(image_preview)).set_texture(cx, Some(texture));
            }
            FilePreview::Text(tp) if tp.is_code => {
                page_flip.set_active_page(cx, id!(code_text_page));
                self.code_view(cx, ids!(code_preview)).set_text(cx, &tp.content);
                self.label(cx, ids!(code_truncated_label)).set_visible(cx, tp.truncated);
            }
            FilePreview::Text(tp) => {
                page_flip.set_active_page(cx, id!(plain_text_page));
                self.code_view(cx, ids!(plain_preview)).set_text(cx, &tp.content);
                self.label(cx, ids!(plain_truncated_label)).set_visible(cx, tp.truncated);
            }
            FilePreview::None => {
                page_flip.set_active_page(cx, id!(no_preview_page));
            }
        }
    }

    /// Clears state and frees every preview resource (texture, text, source file).
    fn reset(&mut self, cx: &mut Cx) {
        self.upload = None;
        self.preview_id = None;
        self.image(cx, ids!(image_preview)).set_texture(cx, None);
        self.code_view(cx, ids!(code_preview)).set_text(cx, "");
        self.code_view(cx, ids!(plain_preview)).set_text(cx, "");
        // Back to the loading page so a reopen can't flash stale content.
        self.page_flip(cx, ids!(preview_flip)).set_active_page(cx, id!(loading_page));
    }
}

impl FileUploadModalRef {
    pub fn handle_file_previewer_action(&self, cx: &mut Cx, outer_modal: ModalRef, action: &FileUploadModalAction) {
        match action {
            FileUploadModalAction::Show { upload, preview_id } => {
                if let Some(mut inner) = self.borrow_mut() {
                    inner.set_upload(cx, upload.clone(), *preview_id);
                }
                outer_modal.open(cx);
            }
            FileUploadModalAction::PreviewReady { preview_id, preview } => {
                // Take from the payload so the decoded image isn't cloned.
                if let Some(preview) = preview.take() {
                    if let Some(mut inner) = self.borrow_mut() {
                        inner.set_preview(cx, *preview_id, preview);
                    }
                }
            }
            FileUploadModalAction::Hide => outer_modal.close(cx),
            FileUploadModalAction::None => {}
        }
    }
}


fn next_file_preview_id() -> FileUploadAttemptId {
    static NEXT_FILE_PREVIEW_ID: AtomicU64 = AtomicU64::new(1);
    NEXT_FILE_PREVIEW_ID.fetch_add(1, Ordering::Relaxed)
}

/// Inspects the given picked file and returns upload-related info about it.
///
/// Note: do not run this on the main UI thread, as it may do expensive operations
///       like reading files, scanning file data for strings, and decoding images.
pub fn load_file_metadata(
    source: robius_file_picker::LocalFile,
    timeline_kind: TimelineKind,
    in_reply_to: Option<OwnedEventId>,
    #[cfg(feature = "tsp")]
    sign_with_tsp: bool,
) -> Result<(AttachmentUpload, PreviewSource, FileUploadAttemptId), String> {
    let path = source.path();
    let metadata = std::fs::metadata(path).map_err(
        |e| format!("Unable to access file: {e}")
    )?;
    if !metadata.is_file() {
        return Err("Cannot upload directories or special files".to_string());
    }
    let file_size = metadata.len();
    // We can trust a platform-provided MIME type, but not one that we guessed.
    let (mime_type, is_mime_guaranteed) = match source
        .mime_type()
        .filter(|m| !m.is_empty() && *m != "application/octet-stream")
    {
        Some(m) => (m.to_owned(), true),
        None => (mime_guess::from_path(path).first_or_octet_stream().to_string(), false),
    };

    let preview_source = PreviewSource {
        // Keep the source `LocalFile` object alive throughout the preview creation.
        source: source.clone(),
        mime_type: mime_type.clone(),
        file_size,
        is_mime_guaranteed,
    };

    let upload = AttachmentUpload {
        timeline_kind,
        file_data: FileUploadMetadata {
            source,
            caption: None,
            mime_type,
            preview: FilePreview::Loading,
            size: file_size,
        },
        in_reply_to,
        #[cfg(feature = "tsp")]
        sign_with_tsp,
    };
    Ok((upload, preview_source, next_file_preview_id()))
}

/// Info needed to create a preview of a file.
pub struct PreviewSource {
    source: robius_file_picker::LocalFile,
    mime_type: String,
    file_size: u64,
    is_mime_guaranteed: bool,
}

impl PreviewSource {
    /// Creates a preview of this file by reading it, figuring out what kind of file it is,
    /// and generating a text, code, image, or other preview.
    ///
    /// Note: do not run this on the main UI thread, as it may do expensive operations
    ///       like reading files, scanning file data for strings, and decoding images.
    pub fn build(self) -> FilePreview {
        let path = self.source.path();
        if crate::image_utils::is_displayable_image(&self.mime_type) {
            match std::fs::read(path) {
                Ok(data) => match decode_image_from_data(&data) {
                    Ok(buffer) => return FilePreview::Image(buffer),
                    Err(e) => {
                        makepad_widgets::warning!("Unable to decode image preview for {path:?}: {e:?}");
                        return FilePreview::None;
                    }
                },
                Err(e) => {
                    makepad_widgets::warning!("Unable to read image preview for {path:?}: {e}");
                    return FilePreview::None;
                }
            }
        }
        match read_text_preview(path, &self.mime_type, self.file_size, self.is_mime_guaranteed) {
            Some(text_preview) => FilePreview::Text(text_preview),
            None => FilePreview::None,
        }
    }
}

/// Reads a subset of the text file (max 128KB) and returns it as a string.
///
/// Returns `None` for non-text files, or upon any other error.
fn read_text_preview(
    path: &std::path::Path,
    mime_type: &str,
    file_size: u64,
    is_mime_guaranteed: bool,
) -> Option<TextPreview> {
    if !crate::utils::mimetype_might_be_text(mime_type, is_mime_guaranteed) {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    file.by_ref().take(TEXT_PREVIEW_MAX_BYTES).read_to_end(&mut buf).ok()?;
    let truncated = file_size > buf.len() as u64;
    let content = bytes_to_string_excerpt(&buf, truncated)?;
    if content.trim().is_empty() {
        return None;
    }
    Some(TextPreview { content, truncated, is_code: crate::utils::is_code_file(path) })
}

fn bytes_to_string_excerpt(bytes: &[u8], was_capped: bool) -> Option<String> {
    // binary files often have null bytes, and we don't want to treat those as text
    if bytes.contains(&0) {
        return None;
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => Some(s.to_string()),
        Err(e) if was_capped && e.error_len().is_none() => {
            std::str::from_utf8(&bytes[..e.valid_up_to()]).ok().map(ToString::to_string)
        }
        Err(_) => None,
    }
}
