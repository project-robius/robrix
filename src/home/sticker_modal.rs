//! Sticker pack catalog modal.
//!
//! Shows a loading spinner while we fetch sticker pack assets from
//! `integrations.element.io` (the public scalar/integrations widget API),
//! then renders the returned packs as a vertical, scrollable list of rows
//! with a pill-shaped toggle on the right edge of each row.
//!
//! The async network plumbing lives in this module — the worker thread in
//! [`crate::sliding_sync`] just forwards `MatrixRequest::LoadStickerCatalog`
//! here so the widget and its data fetcher stay co-located.
//!
//! All scalar-token state (the bearer token Element's widget API issues in
//! exchange for a Matrix OpenID token) is cached at
//! `<app_data_dir>/sticker/scalar_token.json` so we only pay the OpenID
//! handshake once per machine.
//!
//! NOTE: The activation toggle currently affects in-memory state only.
//! Mirroring it back to `m.widgets` (so Element/other clients pick it up)
//! is a deliberate follow-up — none of that matters until rendering itself
//! is verified.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use makepad_widgets::*;
use matrix_sdk::Client;
use serde::{Deserialize, Serialize};

use crate::{LivePtr, widget_ref_from_live_ptr};
use crate::shared::popup_list::{PopupKind, enqueue_popup_notification};


const WIDGET_TYPE: &str = "m.stickerpicker";
const SCALAR_REGISTER_URL: &str = "https://scalar.vector.im/api/register?v=1.1";
const SCALAR_ACCOUNT_URL: &str = "https://integrations.element.io/api/account";
const SCALAR_WIDGETS_REQUEST_URL: &str = "https://integrations.element.io/api/widgets/request";
const SCALAR_WIDGETS_ASSETS_URL: &str = "https://integrations.element.io/api/widgets/assets";
const SCALAR_WIDGETS_PURCHASE_ASSET_URL: &str =
    "https://integrations.element.io/api/widgets/purchase_asset";
const SCALAR_WIDGETS_SET_STATE_URL: &str =
    "https://integrations.element.io/api/widgets/set_asset_state";


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // Dynamic list of sticker pack rows. The `item` field is the template
    // used by `StickerPackList::set_packs` to spawn one row per pack.
    mod.widgets.StickerPackList = #(StickerPackList::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down
        spacing: 0
        padding: Inset{top: 8, bottom: 8}

        // The pack-row template. Layout:
        //   [thumbnail 56x56] [name + description (Fill)] [toggle pill 44x26]
        item: RoundedView {
            width: Fill, height: Fit
            flow: Right
            align: Align{y: 0.5}
            spacing: 12
            padding: Inset{left: 14, right: 14, top: 12, bottom: 12}
            margin: Inset{left: 8, right: 8, top: 6, bottom: 6}

            show_bg: true
            draw_bg +: {
                color: (COLOR_PRIMARY)
                border_radius: 10.0
                border_size: 1.0
                border_color: #xE5E7EB
            }

            // Rounded slot that's always visible. Acts as a placeholder
            // while we fetch the actual sticker image, and as a safe
            // background if the fetch fails entirely.
            thumb_slot := RoundedView {
                width: 56, height: 56
                flow: Overlay
                show_bg: true
                draw_bg +: {
                    color: #xEDEFF3
                    border_radius: 8.0
                }
                thumb := Image {
                    fit: ImageFit.Stretch,
                    width: Fill, height: Fill
                }
            }

            text_col := View {
                width: Fill, height: Fit
                flow: Down
                spacing: 4

                name_label := Label {
                    width: Fill, height: Fit
                    flow: Flow.Right{wrap: true}
                    draw_text +: {
                        text_style: TITLE_TEXT { font_size: 13 }
                        color: #x111111
                    }
                    text: ""
                }

                description_label := Label {
                    width: Fill, height: Fit
                    flow: Flow.Right{wrap: true}
                    draw_text +: {
                        text_style: REGULAR_TEXT { font_size: 11 }
                        color: #x6B7280
                    }
                    text: ""
                }
            }

            // Pill toggle (built-in Makepad widget).
            toggle_pill := ToggleFlat {
                text: ""
            }

        }
    }

    // Grid of individual sticker images shown when the user drills into a pack.
    mod.widgets.StickerGrid = #(StickerGrid::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true}
        spacing: 8
        padding: Inset{left: 8, right: 8, top: 8, bottom: 8}

        // Per-sticker image tile template.
        sticker_item: RoundedView {
            width: 72, height: 72
            flow: Overlay
            show_bg: true
            draw_bg +: {
                color: #xEDEFF3
                border_radius: 8.0
            }

            sticker_img := Image {
                fit: ImageFit.Stretch
                width: Fill, height: Fill
            }

            // Transparent overlay so the whole tile registers as clickable.
            sticker_click_btn := Button {
                width: Fill, height: Fill
                draw_bg: { color: #0000 }
                draw_text: { text_style: {} }
                text: ""
            }
        }
    }

    // Scrollable list of pack sections shown in the Stickers tab.
    // Each section has a pack-name label and a StickerGrid.
    mod.widgets.StickerPackSectionList = #(StickerPackSectionList::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down
        spacing: 12
        padding: Inset{top: 4}

        section_item: View {
            width: Fill, height: Fit
            flow: Down
            spacing: 4

            section_label := Label {
                width: Fill, height: Fit
                padding: Inset{left: 4, bottom: 2}
                draw_text +: {
                    text_style: TITLE_TEXT { font_size: 14 }
                    color: #x111111
                }
                text: ""
            }

            section_grid := mod.widgets.StickerGrid {}
        }
    }

    mod.widgets.StickerModal = set_type_default() do #(StickerModal::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill { max: 560 }
        height: Fill { max: FitBound.Rel{base: Base.Full, factor: 0.85} }
        margin: 40
        flow: Down
        padding: Inset{top: 16, right: 20, bottom: 16, left: 24}
        spacing: 0

        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 8.0
            border_size: 0.0
        }

        // ── Modal header: title + close button ───────────────────────────────
        modal_header := View {
            width: Fill, height: Fit
            flow: Right
            align: Align{y: 0.5}
            spacing: 10
            padding: Inset{bottom: 10}

            modal_title := Label {
                width: Fill, height: Fit
                draw_text +: {
                    text_style: TITLE_TEXT { font_size: 16 }
                    color: #x111111
                }
                text: "Sticker Packs"
            }

            close_button := RobrixIconButton {
                width: Fit, height: Fit
                padding: 8
                spacing: 0
                icon_walk: Walk{width: 16, height: 16, margin: 0}
                draw_icon.svg: (ICON_CLOSE)
                draw_icon.color: #x666
                draw_bg +: {
                    border_size: 0
                    color: #0000
                    color_hover: #00000015
                    color_down: #00000025
                }
            }
        }

        // ── Tab bar: [Stickers] [Catalog] ────────────────────────────────────
        // Each slot: a Button (tap area) + a 2-px indicator line.
        // Active state = indicator visible + active label shown.
        // Inactive state = indicator hidden + inactive label shown.
        tab_bar := View {
            width: Fill, height: Fit
            flow: Right
            spacing: 0

            // --- Stickers tab ---
            // flow: Overlay so the invisible button covers the label+indicator,
            // making the entire tab slot one big click target.
            stickers_tab_slot := View {
                width: Fill, height: Fit
                flow: Overlay

                // Visual layer: label text + active underline stacked vertically.
                View {
                    width: Fill, height: Fit
                    flow: Down
                    spacing: 0

                    stickers_tab_label_row := View {
                        width: Fill, height: Fit
                        flow: Overlay
                        padding: Inset{top: 10, bottom: 8}

                        stickers_tab_active_label := View {
                            visible: true
                            width: Fill, height: Fit
                            align: Align{x: 0.5}
                            Label {
                                width: Fit, height: Fit
                                draw_text +: {
                                    text_style: TITLE_TEXT { font_size: 13 }
                                    color: (COLOR_ACTIVE_PRIMARY)
                                }
                                text: "Stickers"
                            }
                        }
                        stickers_tab_inactive_label := View {
                            visible: false
                            width: Fill, height: Fit
                            align: Align{x: 0.5}
                            Label {
                                width: Fit, height: Fit
                                draw_text +: {
                                    text_style: REGULAR_TEXT { font_size: 13 }
                                    color: #x9CA3AF
                                }
                                text: "Stickers"
                            }
                        }
                    }

                    // Active underline
                    stickers_tab_indicator := View {
                        visible: true
                        width: Fill, height: 2
                        show_bg: true
                        draw_bg +: { color: (COLOR_ACTIVE_PRIMARY) }
                    }
                }

                // Click-capture layer: fills the whole slot on top of the visuals.
                stickers_tab_btn := Button {
                    width: Fill, height: Fill
                    text: ""
                    draw_bg +: {
                        color: #0000
                        border_size: 0
                        color_hover: #00000008
                        color_down: #00000015
                        border_radius: 0.0
                    }
                    draw_text +: { color: #0000 }
                }
            }

            // --- Catalog tab ---
            // Same overlay pattern as the Stickers tab.
            catalog_tab_slot := View {
                width: Fill, height: Fit
                flow: Overlay

                // Visual layer: label text + underline indicator stacked vertically.
                View {
                    width: Fill, height: Fit
                    flow: Down
                    spacing: 0

                    catalog_tab_label_row := View {
                        width: Fill, height: Fit
                        flow: Overlay
                        padding: Inset{top: 10, bottom: 8}

                        catalog_tab_active_label := View {
                            visible: false
                            width: Fill, height: Fit
                            align: Align{x: 0.5}
                            Label {
                                width: Fit, height: Fit
                                draw_text +: {
                                    text_style: TITLE_TEXT { font_size: 13 }
                                    color: (COLOR_ACTIVE_PRIMARY)
                                }
                                text: "Catalog"
                            }
                        }
                        catalog_tab_inactive_label := View {
                            visible: true
                            width: Fill, height: Fit
                            align: Align{x: 0.5}
                            Label {
                                width: Fit, height: Fit
                                draw_text +: {
                                    text_style: REGULAR_TEXT { font_size: 13 }
                                    color: #x9CA3AF
                                }
                                text: "Catalog"
                            }
                        }
                    }

                    // Inactive underline (hidden by default)
                    catalog_tab_indicator := View {
                        visible: false
                        width: Fill, height: 2
                        show_bg: true
                        draw_bg +: { color: (COLOR_ACTIVE_PRIMARY) }
                    }
                }

                // Click-capture layer: fills the whole slot on top of the visuals.
                catalog_tab_btn := Button {
                    width: Fill, height: Fill
                    text: ""
                    draw_bg +: {
                        color: #0000
                        border_size: 0
                        color_hover: #00000008
                        color_down: #00000015
                        border_radius: 0.0
                    }
                    draw_text +: { color: #0000 }
                }
            }
        }

        // Full-width separator below tabs
        View {
            width: Fill, height: 1
            show_bg: true
            draw_bg +: { color: #xE5E7EB }
        }

        // ── Content panes: Overlay — only one visible at a time ──────────────
        content_panes := View {
            width: Fill, height: Fill
            flow: Overlay

            // ----- Sticker pane (default) -----
            sticker_pane := View {
                width: Fill, height: Fill
                flow: Down
                spacing: 10
                padding: Inset{top: 10}

                // Spinner while bytes load
                picker_loading := View {
                    visible: false
                    width: Fill, height: Fill
                    flow: Down
                    align: Align{x: 0.5, y: 0.5}
                    spacing: 14

                    LoadingSpinner {
                        width: 32, height: 32
                        draw_bg.color: (COLOR_ACTIVE_PRIMARY)
                    }
                    Label {
                        width: Fit, height: Fit
                        draw_text +: {
                            text_style: REGULAR_TEXT { font_size: 12 }
                            color: #x6B7280
                        }
                        text: "Loading stickers…"
                    }
                }

                // All active pack sections
                picker_scroll := ScrollYView {
                    visible: false
                    width: Fill, height: Fill
                    flow: Down

                    pack_sections := mod.widgets.StickerPackSectionList {}
                }
            }

            // ----- Catalog pane (hidden until tab switched) -----
            catalog_pane := View {
                visible: false
                width: Fill, height: Fill
                flow: Down
                spacing: 0
                padding: Inset{top: 10}

                // ··· all the existing catalog body states ···

                loading_state := View {
                    width: Fill, height: Fill
                    flow: Down
                    align: Align{x: 0.5, y: 0.5}
                    spacing: 14

                    loading_spinner := LoadingSpinner {
                        width: 36, height: 36
                        draw_bg.color: (COLOR_ACTIVE_PRIMARY)
                    }
                    loading_label := Label {
                        width: Fit, height: Fit
                        draw_text +: {
                            text_style: REGULAR_TEXT { font_size: 12 }
                            color: #x6B7280
                        }
                        text: "Loading sticker packs…"
                    }
                }

                loaded_state := View {
                    visible: false
                    width: Fill, height: Fill
                    flow: Down

                    catalog_scroll := ScrollYView {
                        width: Fill, height: Fill
                        flow: Down
                        spacing: 0

                        pack_list := mod.widgets.StickerPackList {}
                    }
                }

                error_state := View {
                    visible: false
                    width: Fill, height: Fill
                    flow: Down
                    align: Align{x: 0.5, y: 0.5}
                    spacing: 12
                    padding: Inset{left: 24, right: 24}

                    error_label := Label {
                        width: Fill, height: Fit
                        flow: Flow.Right{wrap: true}
                        align: Align{x: 0.5, y: 0.5}
                        draw_text +: {
                            text_style: REGULAR_TEXT { font_size: 12 }
                            color: #xB91C1C
                        }
                        text: "Failed to load sticker packs."
                    }
                    retry_button := Button {
                        width: Fit, height: Fit
                        padding: Inset{top: 6, bottom: 6, left: 14, right: 14}
                        text: "Retry"
                        draw_bg +: {
                            color: (COLOR_ACTIVE_PRIMARY)
                            color_hover: (COLOR_ACTIVE_PRIMARY_DARKER)
                            color_down: #0C5DAA
                            border_radius: 4.0
                        }
                        draw_text +: {
                            color: #fff
                            text_style: REGULAR_TEXT { font_size: 11 }
                        }
                    }
                }

                sticker_grid_state := View {
                    visible: false
                    width: Fill, height: Fill
                    flow: Down
                    spacing: 0

                    sticker_grid_header := View {
                        width: Fill, height: Fit
                        flow: Right
                        align: Align{y: 0.5}
                        spacing: 6
                        padding: Inset{bottom: 8}

                        sticker_back_btn := RobrixIconButton {
                            width: Fit, height: Fit
                            padding: 8
                            spacing: 0
                            icon_walk: Walk{width: 16, height: 16, margin: 0}
                            draw_icon.svg: (ICON_JUMP)
                            draw_icon.color: #x444
                            draw_bg +: {
                                border_size: 0
                                color: #0000
                                color_hover: #00000015
                                color_down: #00000025
                            }
                        }
                        grid_pack_name := Label {
                            width: Fill, height: Fit
                            draw_text +: {
                                text_style: TITLE_TEXT { font_size: 15 }
                                color: #x111111
                            }
                            text: ""
                        }
                    }

                    View {
                        width: Fill, height: 1
                        show_bg: true
                        draw_bg +: { color: #xE5E7EB }
                    }

                    sticker_grid_loading := View {
                        visible: true
                        width: Fill, height: Fill
                        flow: Down
                        align: Align{x: 0.5, y: 0.5}
                        spacing: 12

                        LoadingSpinner {
                            width: 28, height: 28
                            draw_bg.color: (COLOR_ACTIVE_PRIMARY)
                        }
                        Label {
                            width: Fit, height: Fit
                            draw_text +: {
                                text_style: REGULAR_TEXT { font_size: 11 }
                                color: #x6B7280
                            }
                            text: "Loading stickers…"
                        }
                    }

                    sticker_grid_scroll := ScrollYView {
                        visible: false
                        width: Fill, height: Fill
                        flow: Down

                        sticker_grid := mod.widgets.StickerGrid {}
                    }
                }
            }
        }
    }
}


// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// Drives modal visibility from elsewhere in the app
/// (typically the sticker drawer in `room_input_bar.rs`).
#[derive(Clone, Debug)]
pub enum StickerModalAction {
    /// Open the modal and kick off a catalog fetch.
    Open,
    /// Open the modal showing only the Stickers tab (no catalog visible).
    OpenStickersOnly,
    /// Close the modal (does NOT cancel an in-flight fetch — the result
    /// will just be ignored if it arrives at a hidden modal).
    Close,
}

/// Worker → UI: result of `MatrixRequest::LoadStickerCatalog`.
#[derive(Clone, Debug)]
pub enum StickerCatalogAction {
    Ready {
        packs: Vec<StickerPack>,
    },
    Failed {
        /// User-displayable error message.
        error: String,
    },
}

/// Worker → UI: individual sticker images for one pack, loaded on demand.
#[derive(Clone, Debug)]
pub enum StickerGridAction {
    Ready {
        pack_id: String,
        pack_name: String,
        stickers: Vec<StickerImage>,
    },
    Failed {
        pack_id: String,
        error: String,
    },
}

/// One sticker pack as displayed in the catalog list.
///
/// Field names mirror the scalar `/api/widgets/assets` response shape
/// (`AssetCollection { id, assets: [Asset { asset_type, name, description,
/// thumbnail, purchased, .. }] }`), with `thumbnail_bytes` populated after
/// a follow-up HTTP fetch.
#[derive(Clone, Debug)]
pub struct StickerPack {
    /// UI-side identity. Synthesized from `<collection_id>_<asset_type>`
    /// when both are available so it stays stable across reloads.
    pub id: String,
    /// The `asset_type` field on the wire. This is what `set_asset_state`
    /// expects as its `asset_type` query parameter (e.g. `"isabella"`).
    pub asset_type: String,
    pub name: String,
    pub description: String,
    /// Resolved (absolute) thumbnail URL. May be empty.
    pub thumbnail_url: String,
    /// Decoded PNG/JPEG bytes. Populated by the worker after fetching
    /// `thumbnail_url`; empty if the fetch failed or the URL was missing.
    pub thumbnail_bytes: Vec<u8>,
    /// Mirrors `Asset::purchased` on the wire.
    pub is_active: bool,
    /// Individual stickers within this pack.  Parsed from `data.stickers[]`
    /// in the `/api/widgets/assets` response.  `image_bytes` is populated
    /// lazily — only when the user drills into the pack.
    pub stickers: Vec<StickerImage>,
}

/// One individual sticker image within a pack.
#[derive(Clone, Debug)]
pub struct StickerImage {
    /// Human-readable label (the `body` field on the wire).
    pub body: String,
    /// Original `mxc://` URL as returned by the API.
    pub url: String,
    /// HTTPS URL derived from `url` — the address actually used for fetching.
    /// Empty when the URL could not be resolved.
    pub https_url: String,
    /// Image width in pixels, parsed from the PNG header once bytes are loaded.
    pub width: u32,
    /// Image height in pixels, parsed from the PNG header once bytes are loaded.
    pub height: u32,
    /// Decoded image bytes.  Populated by `load_pack_stickers`; empty until
    /// the user opens the sticker grid for this pack.
    pub image_bytes: Vec<u8>,
}


// ---------------------------------------------------------------------------
// StickerPackList — custom dynamic-children widget (event_reaction_list pattern)
// ---------------------------------------------------------------------------

#[derive(Script, ScriptHook, Widget)]
pub struct StickerPackList {
    #[uid] uid: WidgetUid,
    #[redraw] #[rust] area: Area,
    #[live] item: Option<LivePtr>,
    #[rust] children: Vec<(WidgetRef, StickerPack)>,
    #[layout] layout: Layout,
    #[walk] walk: Walk,
}

impl Widget for StickerPackList {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        for (row, _) in self.children.iter_mut() {
            let _ = row.draw(cx, scope);
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Let each row's inner widgets (including the ToggleFlat) process
        // the event so they emit standard CheckBox actions.
        for (row, _) in self.children.iter_mut() {
            row.handle_event(cx, event, scope);
        }
        if let Event::Actions(actions) = event {
            for (row, pack) in self.children.iter_mut() {
                if let Some(new_state) =
                    row.check_box(cx, ids!(toggle_pill)).changed(actions)
                {
                    pack.is_active = new_state;
                    log!(
                        "[sticker] toggle clicked id={} asset_type={} → {}",
                        pack.id,
                        pack.asset_type,
                        if new_state { "ON (set_asset_state enable)" } else { "OFF (set_asset_state disable)" },
                    );
                    if !pack.asset_type.is_empty() {
                        crate::sliding_sync::submit_async_request(
                            crate::sliding_sync::MatrixRequest::SetStickerPackState {
                                asset_type: pack.asset_type.clone(),
                                enable: new_state,
                            },
                        );
                    } else {
                        log!(
                            "[sticker] toggle clicked but asset_type is empty for id={}; \
                             skipping network call",
                            pack.id
                        );
                    }
                    cx.widget_action(
                        self.uid,
                        StickerPackToggleAction {
                            id: pack.id.clone(),
                            is_active: new_state,
                        },
                    );
                }

            }
        }
    }
}

impl StickerPackList {
    fn set_packs(&mut self, cx: &mut Cx, packs: Vec<StickerPack>) {
        log!("[sticker] rendering {} pack row(s) into the catalog", packs.len());
        self.children.clear();
        for pack in packs {
            let row = widget_ref_from_live_ptr(cx, self.item);
            row.label(cx, ids!(name_label)).set_text(cx, &pack.name);
            row.label(cx, ids!(description_label)).set_text(cx, &pack.description);
            let thumb = row.image(cx, ids!(thumb));
            if pack.thumbnail_bytes.is_empty() {
                log!(
                    "[sticker]   id={} → no thumbnail bytes; placeholder will show",
                    pack.id
                );
            } else {
                match crate::utils::load_png_or_jpg(&thumb, cx, &pack.thumbnail_bytes) {
                    Ok(()) => log!(
                        "[sticker]   id={} → decoded {} byte thumbnail",
                        pack.id,
                        pack.thumbnail_bytes.len()
                    ),
                    Err(e) => log!(
                        "[sticker]   id={} → could not decode thumbnail ({} bytes): {e:?}",
                        pack.id,
                        pack.thumbnail_bytes.len()
                    ),
                }
                thumb.redraw(cx);
            }
            row.check_box(cx, ids!(toggle_pill))
                .set_active(cx, pack.is_active);
            self.children.push((row, pack));
        }
    }
}

impl StickerPackListRef {
    pub fn set_packs(&self, cx: &mut Cx, packs: Vec<StickerPack>) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_packs(cx, packs);
    }
}


// ---------------------------------------------------------------------------
// StickerGrid — dynamic image grid (event_reaction_list pattern)
// ---------------------------------------------------------------------------

#[derive(Script, ScriptHook, Widget)]
pub struct StickerGrid {
    #[uid] uid: WidgetUid,
    #[redraw] #[rust] area: Area,
    #[live] sticker_item: Option<LivePtr>,
    #[rust] children: Vec<(WidgetRef, StickerImage)>,
    #[layout] layout: Layout,
    #[walk] walk: Walk,
}

impl Widget for StickerGrid {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        for (item, _) in self.children.iter_mut() {
            let _ = item.draw(cx, scope);
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        for (item, _) in self.children.iter_mut() {
            item.handle_event(cx, event, scope);
        }
        if let Event::Actions(actions) = event {
            for (item, sticker) in self.children.iter() {
                if item.button(cx, ids!(sticker_click_btn)).clicked(actions) {
                    log!("[sticker-dbg] LAYER1: sticker_click_btn clicked body={:?} url={:?}", sticker.body, sticker.url);
                    cx.action(StickerModalAction::Close);
                    cx.action(StickerSendAction { sticker: sticker.clone() });
                    break;
                }
            }
        }
    }
}

impl StickerGrid {
    fn set_stickers(&mut self, cx: &mut Cx, stickers: Vec<StickerImage>) {
        log!("[sticker] rendering {} sticker image(s) into grid", stickers.len());
        self.children.clear();
        for sticker in stickers {
            let item = widget_ref_from_live_ptr(cx, self.sticker_item);
            let img = item.image(cx, ids!(sticker_img));
            if sticker.image_bytes.is_empty() {
                log!(
                    "[sticker]   grid sticker body={:?} → no bytes; placeholder visible",
                    sticker.body
                );
            } else {
                match crate::utils::load_png_or_jpg(&img, cx, &sticker.image_bytes) {
                    Ok(()) => log!(
                        "[sticker]   grid sticker body={:?} decoded {} bytes",
                        sticker.body,
                        sticker.image_bytes.len()
                    ),
                    Err(e) => log!(
                        "[sticker]   grid sticker body={:?} decode error: {e:?}",
                        sticker.body
                    ),
                }
                img.redraw(cx);
            }
            self.children.push((item, sticker));
        }
    }
}

impl StickerGrid {
    /// Update individual tiles that arrived after the initial `Ready` action.
    /// Only tiles whose index is in `updates` are re-decoded and redrawn.
    fn patch_stickers(&mut self, cx: &mut Cx, updates: Vec<(usize, Vec<u8>)>) {
        for (idx, bytes) in updates {
            let Some((item, sticker)) = self.children.get_mut(idx) else { continue };
            let (w, h) = parse_png_dimensions(&bytes);
            sticker.image_bytes = bytes.clone();
            sticker.width = w;
            sticker.height = h;
            if !bytes.is_empty() {
                let img = item.image(cx, ids!(sticker_img));
                match crate::utils::load_png_or_jpg(&img, cx, &bytes) {
                    Ok(()) => img.redraw(cx),
                    Err(e) => log!(
                        "[sticker] patch decode error idx={idx}: {e:?}"
                    ),
                }
            }
        }
    }
}

impl StickerGridRef {
    pub fn set_stickers(&self, cx: &mut Cx, stickers: Vec<StickerImage>) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_stickers(cx, stickers);
    }

    pub fn patch_stickers(&self, cx: &mut Cx, updates: Vec<(usize, Vec<u8>)>) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.patch_stickers(cx, updates);
    }
}

// ---------------------------------------------------------------------------
// StickerPackSectionList — one section (label + grid) per active pack
// ---------------------------------------------------------------------------

#[derive(Script, ScriptHook, Widget)]
pub struct StickerPackSectionList {
    #[uid] uid: WidgetUid,
    #[redraw] #[rust] area: Area,
    #[live] section_item: Option<LivePtr>,
    #[rust] children: Vec<(WidgetRef, String)>,
    #[layout] layout: Layout,
    #[walk] walk: Walk,
}

impl Widget for StickerPackSectionList {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        for (item, _) in self.children.iter_mut() {
            let _ = item.draw(cx, scope);
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        for (item, _) in self.children.iter_mut() {
            item.handle_event(cx, event, scope);
        }
    }
}

impl StickerPackSectionList {
    fn add_or_update_section(
        &mut self,
        cx: &mut Cx,
        pack_id: &str,
        pack_name: &str,
        stickers: Vec<StickerImage>,
    ) {
        if let Some(pos) = self.children.iter().position(|(_, id)| id == pack_id) {
            let (item, _) = &self.children[pos];
            item.sticker_grid(cx, ids!(section_grid)).set_stickers(cx, stickers);
            return;
        }
        let item = widget_ref_from_live_ptr(cx, self.section_item);
        item.label(cx, ids!(section_label)).set_text(cx, pack_name);
        item.sticker_grid(cx, ids!(section_grid)).set_stickers(cx, stickers);
        self.children.push((item, pack_id.to_string()));
    }

    fn patch_section(&mut self, cx: &mut Cx, pack_id: &str, updates: Vec<(usize, Vec<u8>)>) {
        if let Some((item, _)) = self.children.iter().find(|(_, id)| id == pack_id) {
            item.sticker_grid(cx, ids!(section_grid)).patch_stickers(cx, updates);
        }
    }
}

impl StickerPackSectionListRef {
    pub fn add_or_update_section(
        &self,
        cx: &mut Cx,
        pack_id: &str,
        pack_name: &str,
        stickers: Vec<StickerImage>,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.add_or_update_section(cx, pack_id, pack_name, stickers);
    }

    pub fn patch_section(&self, cx: &mut Cx, pack_id: &str, updates: Vec<(usize, Vec<u8>)>) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.patch_section(cx, pack_id, updates);
    }

    pub fn remove_section(&self, pack_id: &str) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.children.retain(|(_, id)| id != pack_id);
    }
}

/// Per-row widget action emitted when a pack's toggle is clicked.
#[derive(Clone, Debug)]
pub struct StickerPackToggleAction {
    pub id: String,
    pub is_active: bool,
}

/// Action emitted when the user taps the "▸" (view stickers) button for a
/// pack.  Carries enough info to dispatch `MatrixRequest::LoadPackStickers`.
#[derive(Clone, Debug)]
pub struct StickerPackClickAction {
    pub pack_id: String,
    pub pack_name: String,
    /// `(mxc_url, https_url, body)` tuples for all stickers in the pack.
    pub sticker_infos: Vec<(String, String, String)>,
}

/// Emitted by `StickerGrid` when the user taps a sticker tile.
#[derive(Clone, Debug)]
pub struct StickerTappedAction {
    pub sticker: StickerImage,
}

/// Emitted by `StickerModal` after the user selects a sticker.
/// `room_input_bar` catches this to send the sticker to the current room.
#[derive(Clone, Debug)]
pub struct StickerSendAction {
    pub sticker: StickerImage,
}

/// Posted by `load_pack_stickers_streaming` after a batch of sticker images
/// arrives from the network (i.e. was not already in the disk cache).
/// The UI merges these bytes into the already-displayed tile grid so the user
/// sees tiles fill in progressively rather than waiting for every image.
#[derive(Clone, Debug)]
pub struct StickerImagePatchAction {
    /// Identifies which pack this patch belongs to.
    pub pack_id: String,
    /// `(sticker_index, image_bytes)` — only entries with non-empty bytes.
    pub updates: Vec<(usize, Vec<u8>)>,
}


// ---------------------------------------------------------------------------
// StickerModal
// ---------------------------------------------------------------------------

/// Which tab is currently visible.
#[derive(Clone, Copy, Default, PartialEq)]
enum ActiveTab {
    /// Sticker tile picker for the first active pack (shown by default when a
    /// cached active pack with sticker URLs is available).
    #[default]
    Stickers,
    /// Full pack catalog — list of all available packs with activation toggles.
    Catalog,
}

/// State of the **catalog** panel.
/// Updated even while the catalog panel is hidden so its content is ready
/// when the user switches to it.
#[derive(Default)]
enum ModalState {
    #[default]
    Loading,
    Loaded,
    Error,
    /// Drill-down grid for a single pack (triggered by the "▸" row button).
    StickerGrid,
}

#[derive(Script, ScriptHook, Widget)]
pub struct StickerModal {
    #[deref] view: View,
    /// State of the catalog panel's sub-views.
    #[rust] state: ModalState,
    /// Which of the two tab panels is currently visible.
    #[rust] active_tab: ActiveTab,
    /// Pack name shown in the catalog drill-down header.
    #[rust] current_pack_name: String,
    /// Last successfully fetched pack list; used to populate the catalog panel
    /// without re-fetching when the user switches tabs.
    #[rust] cached_packs: Vec<StickerPack>,
    /// Pack ids currently displayed in the **sticker pane** (tab 1).
    /// Used to route `StickerImagePatchAction` to the right section.
    #[rust] sticker_pane_pack_ids: Vec<String>,
    /// Pack id currently displayed in the **catalog drill-down** (tab 2).
    #[rust] catalog_drill_pack_id: String,
    /// When true, the tab bar is hidden (stickers-only mode).
    #[rust] stickers_only_mode: bool,
}

impl Widget for StickerModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for StickerModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        // ── Modal-level close ────────────────────────────────────────────────
        if self.view.button(cx, ids!(close_button)).clicked(actions) {
            cx.action(StickerModalAction::Close);
            return;
        }

        // The Modal wrapper emits Dismissed when the user clicks outside.
        // We must NOT re-emit StickerModalAction::Close (the modal already
        // closed itself; re-emitting would loop).
        if actions
            .iter()
            .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            return;
        }

        // ── Tab bar buttons ──────────────────────────────────────────────────
        if self.view.button(cx, ids!(stickers_tab_btn)).clicked(actions) {
            self.set_active_tab(cx, ActiveTab::Stickers);
            // If the sticker pane hasn't loaded yet (e.g. user started on
            // Catalog tab), kick off loading now or show an idle empty state.
            if self.sticker_pane_pack_ids.is_empty() {
                self.ensure_sticker_pane_loaded(cx);
            }
            return;
        }
        if self.view.button(cx, ids!(catalog_tab_btn)).clicked(actions) {
            self.set_active_tab(cx, ActiveTab::Catalog);
            // If the catalog hasn't loaded yet, kick off a request.
            if matches!(self.state, ModalState::Loading) && self.cached_packs.is_empty() {
                crate::sliding_sync::submit_async_request(
                    crate::sliding_sync::MatrixRequest::LoadStickerCatalog,
                );
            }
            return;
        }


        // ── Catalog drill-down back button ───────────────────────────────────
        if self.view.button(cx, ids!(sticker_back_btn)).clicked(actions) {
            self.show_catalog(cx);
            return;
        }

        // ── Retry button in catalog error state ──────────────────────────────
        if self.view.button(cx, ids!(retry_button)).clicked(actions) {
            self.set_catalog_loading(cx);
            crate::sliding_sync::submit_async_request(
                crate::sliding_sync::MatrixRequest::LoadStickerCatalog,
            );
        }

        // ── Async action results ─────────────────────────────────────────────
        for action in actions {
            // User tapped a sticker tile → close modal and broadcast send action.
            if let Some(tapped) = action.downcast_ref::<StickerTappedAction>() {
                println!("StickerTappedAction2");
                cx.action(StickerModalAction::Close);
                cx.action(StickerSendAction { sticker: tapped.sticker.clone() });
                return;
            }

            // Sync cached_packs when the user toggles a pack in the catalog UI.
            if let Some(toggle) = action.downcast_ref::<StickerPackToggleAction>() {
                if let Some(pack) = self.cached_packs.iter_mut().find(|p| p.id == toggle.id) {
                    pack.is_active = toggle.is_active;
                }
                if toggle.is_active {
                    // Refresh catalog to get sticker URLs for the newly enabled pack.
                    crate::sliding_sync::submit_async_request(
                        crate::sliding_sync::MatrixRequest::LoadStickerCatalog,
                    );
                } else {
                    // Remove the pack's section from the Stickers pane immediately.
                    self.sticker_pane_pack_ids.retain(|id| id != &toggle.id);
                    self.view
                        .sticker_pack_section_list(cx, ids!(pack_sections))
                        .remove_section(&toggle.id);
                    self.view.redraw(cx);
                }
            }

            if let Some(catalog_action) = action.downcast_ref::<StickerCatalogAction>() {
                match catalog_action {
                    StickerCatalogAction::Ready { packs } => {
                        save_catalog_cache(packs);
                        self.cached_packs = packs.clone();
                        self.set_catalog_loaded_state(cx, packs.clone());

                        // Load any newly active packs that aren't yet in the Stickers pane.
                        let existing_ids = self.sticker_pane_pack_ids.clone();
                        let new_to_load: Vec<(String, String, Vec<(String, String, String)>)> =
                            self.cached_packs.iter()
                                .filter(|p| p.is_active && !existing_ids.contains(&p.id))
                                .filter_map(|pack| {
                                    let infos: Vec<(String, String, String)> = pack.stickers.iter()
                                        .filter(|s| !s.https_url.is_empty())
                                        .map(|s| (s.url.clone(), s.https_url.clone(), s.body.clone()))
                                        .collect();
                                    if infos.is_empty() { return None; }
                                    Some((pack.id.clone(), pack.name.clone(), infos))
                                })
                                .collect();

                        for (pack_id, pack_name, infos) in new_to_load {
                            self.sticker_pane_pack_ids.push(pack_id.clone());
                            crate::sliding_sync::submit_async_request(
                                crate::sliding_sync::MatrixRequest::LoadPackStickers {
                                    pack_id,
                                    pack_name,
                                    sticker_infos: infos,
                                },
                            );
                        }
                    }
                    StickerCatalogAction::Failed { error } => {
                        self.set_catalog_error(cx, error);
                    }
                }
            }

            // Pack row "▸" tapped → drill into that pack's sticker grid.
            if let Some(click_action) = action.downcast_ref::<StickerPackClickAction>() {
                // Pre-set the drill pack_id so StickerGridAction::Ready can be
                // routed by pack_id rather than active_tab (race-condition safe).
                self.catalog_drill_pack_id = click_action.pack_id.clone();
                self.show_sticker_grid_loading(cx, &click_action.pack_name.clone());
                crate::sliding_sync::submit_async_request(
                    crate::sliding_sync::MatrixRequest::LoadPackStickers {
                        pack_id: click_action.pack_id.clone(),
                        pack_name: click_action.pack_name.clone(),
                        sticker_infos: click_action.sticker_infos.clone(),
                    },
                );
            }

            // Sticker images loaded — route to the appropriate panel.
            // `StickerGridAction::Ready` may now arrive with placeholder bytes
            // (from the disk-cache phase); `StickerImagePatchAction` fills in
            // the rest incrementally as network fetches complete.
            if let Some(grid_action) = action.downcast_ref::<StickerGridAction>() {
                match grid_action {
                    StickerGridAction::Ready { pack_id, pack_name, stickers } => {
                        if self.sticker_pane_pack_ids.contains(pack_id) {
                            self.set_sticker_pane_loaded(cx, pack_id, pack_name, stickers.clone());
                        } else if *pack_id == self.catalog_drill_pack_id {
                            self.set_sticker_grid_loaded(cx, pack_name, stickers.clone());
                        } else {
                            match self.active_tab {
                                ActiveTab::Stickers => {
                                    self.sticker_pane_pack_ids.push(pack_id.clone());
                                    self.set_sticker_pane_loaded(cx, pack_id, pack_name, stickers.clone());
                                }
                                ActiveTab::Catalog => {
                                    self.catalog_drill_pack_id = pack_id.clone();
                                    self.set_sticker_grid_loaded(cx, pack_name, stickers.clone());
                                }
                            }
                        }
                    }
                    StickerGridAction::Failed { pack_id, error } => {
                        log!("[sticker] sticker load failed pack_id={pack_id}: {error}");
                        if self.sticker_pane_pack_ids.contains(pack_id) {
                            self.view.view(cx, ids!(picker_loading)).set_visible(cx, false);
                            self.view.view(cx, ids!(picker_scroll)).set_visible(cx, true);
                            self.view.redraw(cx);
                        } else if *pack_id == self.catalog_drill_pack_id {
                            self.show_catalog(cx);
                        } else {
                            match self.active_tab {
                                ActiveTab::Stickers => {
                                    self.view.view(cx, ids!(picker_loading)).set_visible(cx, false);
                                    self.view.view(cx, ids!(picker_scroll)).set_visible(cx, true);
                                    self.view.redraw(cx);
                                }
                                ActiveTab::Catalog => {
                                    self.show_catalog(cx);
                                }
                            }
                        }
                    }
                }
            }

            // Progressive patch: individual sticker images arrived from the
            // network after the initial disk-cache render.
            if let Some(patch) = action.downcast_ref::<StickerImagePatchAction>() {
                if self.sticker_pane_pack_ids.contains(&patch.pack_id) {
                    self.view
                        .sticker_pack_section_list(cx, ids!(pack_sections))
                        .patch_section(cx, &patch.pack_id, patch.updates.clone());
                    self.view.redraw(cx);
                }
                if patch.pack_id == self.catalog_drill_pack_id {
                    self.view
                        .sticker_grid(cx, ids!(sticker_grid))
                        .patch_stickers(cx, patch.updates.clone());
                    self.view.redraw(cx);
                }
            }
        }
    }
}

impl StickerModal {
    /// Open the modal showing only the Stickers tab; the tab bar and catalog are hidden.
    pub fn show_stickers_only(&mut self, cx: &mut Cx) {
        self.stickers_only_mode = true;
        self.view.view(cx, ids!(tab_bar)).set_visible(cx, false);
        self.view.label(cx, ids!(modal_title)).set_text(cx, "My Stickers");
        self.set_active_tab(cx, ActiveTab::Stickers);

        if let Some(cached) = load_catalog_cache() {
            let active_packs: Vec<(String, String, Vec<(String, String, String)>)> =
                cached.packs.iter()
                    .filter(|p| p.is_active)
                    .filter_map(|pack| {
                        let infos: Vec<(String, String, String)> = pack.stickers.iter()
                            .filter(|s| !s.https_url.is_empty())
                            .map(|s| (s.mxc_url.clone(), s.https_url.clone(), s.body.clone()))
                            .collect();
                        if infos.is_empty() { return None; }
                        Some((pack.id.clone(), pack.name.clone(), infos))
                    })
                    .collect();

            if !active_packs.is_empty() {
                self.sticker_pane_pack_ids = active_packs.iter().map(|(id, _, _)| id.clone()).collect();
                self.show_sticker_pane_loading(cx);
                for (pack_id, pack_name, infos) in active_packs {
                    crate::sliding_sync::submit_async_request(
                        crate::sliding_sync::MatrixRequest::LoadPackStickers {
                            pack_id,
                            pack_name,
                            sticker_infos: infos,
                        },
                    );
                }
                return;
            }
        }
        self.ensure_sticker_pane_loaded(cx);
    }

    /// Open the modal.
    ///
    /// Strategy:
    /// 1. Disk cache has an active pack with sticker URLs → show **Stickers tab**
    ///    immediately (with a spinner) while image bytes load.  A background
    ///    `LoadStickerCatalog` also runs to refresh the catalog panel.
    /// 2. No usable cache → show **Catalog tab** with a loading spinner and
    ///    start the full scalar fetch flow.
    pub fn show(&mut self, cx: &mut Cx) {
        self.stickers_only_mode = false;
        self.view.view(cx, ids!(tab_bar)).set_visible(cx, true);
        self.view.label(cx, ids!(modal_title)).set_text(cx, "Sticker Packs");
        if let Some(cached) = load_catalog_cache() {
            let active_packs: Vec<(String, String, Vec<(String, String, String)>)> =
                cached.packs.iter()
                    .filter(|p| p.is_active)
                    .filter_map(|pack| {
                        let infos: Vec<(String, String, String)> = pack.stickers.iter()
                            .filter(|s| !s.https_url.is_empty())
                            .map(|s| (s.mxc_url.clone(), s.https_url.clone(), s.body.clone()))
                            .collect();
                        if infos.is_empty() { return None; }
                        Some((pack.id.clone(), pack.name.clone(), infos))
                    })
                    .collect();

            if !active_packs.is_empty() {
                log!("[sticker] cache hit: {} active pack(s) — Stickers tab", active_packs.len());
                self.sticker_pane_pack_ids = active_packs.iter().map(|(id, _, _)| id.clone()).collect();
                self.set_active_tab(cx, ActiveTab::Stickers);
                self.show_sticker_pane_loading(cx);
                for (pack_id, pack_name, infos) in active_packs {
                    crate::sliding_sync::submit_async_request(
                        crate::sliding_sync::MatrixRequest::LoadPackStickers {
                            pack_id,
                            pack_name,
                            sticker_infos: infos,
                        },
                    );
                }
                crate::sliding_sync::submit_async_request(
                    crate::sliding_sync::MatrixRequest::LoadStickerCatalog,
                );
                return;
            }
        }

        // No usable cache → start on Catalog tab with a loading spinner.
        self.set_active_tab(cx, ActiveTab::Catalog);
        self.set_catalog_loading(cx);
        crate::sliding_sync::submit_async_request(
            crate::sliding_sync::MatrixRequest::LoadStickerCatalog,
        );
    }

    // ── Tab management ──────────────────────────────────────────────────────

    fn set_active_tab(&mut self, cx: &mut Cx, tab: ActiveTab) {
        self.active_tab = tab;
        let on_stickers = matches!(tab, ActiveTab::Stickers);
        // Toggle indicators (active underline).
        self.view.view(cx, ids!(stickers_tab_indicator)).set_visible(cx, on_stickers);
        self.view.view(cx, ids!(catalog_tab_indicator)).set_visible(cx, !on_stickers);
        // Toggle active/inactive label variants.
        self.view.view(cx, ids!(stickers_tab_active_label)).set_visible(cx, on_stickers);
        self.view.view(cx, ids!(stickers_tab_inactive_label)).set_visible(cx, !on_stickers);
        self.view.view(cx, ids!(catalog_tab_active_label)).set_visible(cx, !on_stickers);
        self.view.view(cx, ids!(catalog_tab_inactive_label)).set_visible(cx, on_stickers);
        // Toggle content panes.
        self.view.view(cx, ids!(sticker_pane)).set_visible(cx, on_stickers);
        self.view.view(cx, ids!(catalog_pane)).set_visible(cx, !on_stickers);
        self.view.redraw(cx);
    }

    // ── Sticker pane helpers ────────────────────────────────────────────────

    /// Show the sticker pane with a loading spinner while image bytes load.
    fn show_sticker_pane_loading(&mut self, cx: &mut Cx) {
        self.view.view(cx, ids!(picker_loading)).set_visible(cx, true);
        self.view.view(cx, ids!(picker_scroll)).set_visible(cx, false);
        self.view.redraw(cx);
    }

    /// Add or update a pack section in the sticker pane and hide the spinner.
    fn set_sticker_pane_loaded(&mut self, cx: &mut Cx, pack_id: &str, pack_name: &str, stickers: Vec<StickerImage>) {
        self.view
            .sticker_pack_section_list(cx, ids!(pack_sections))
            .add_or_update_section(cx, pack_id, pack_name, stickers);
        self.view.view(cx, ids!(picker_loading)).set_visible(cx, false);
        self.view.view(cx, ids!(picker_scroll)).set_visible(cx, true);
        self.view.redraw(cx);
    }

    /// Start loading the sticker pane from cached packs, or show an idle
    /// empty grid when no active pack is available.
    ///
    /// Called when the user manually switches to the Stickers tab but no
    /// sticker load has been triggered yet (e.g. the modal opened on the
    /// Catalog tab because there was no cache hit on startup).
    fn ensure_sticker_pane_loaded(&mut self, cx: &mut Cx) {
        let active_packs: Vec<(String, String, Vec<(String, String, String)>)> =
            self.cached_packs.iter()
                .filter(|p| p.is_active)
                .filter_map(|pack| {
                    let infos: Vec<(String, String, String)> = pack.stickers.iter()
                        .filter(|s| !s.https_url.is_empty())
                        .map(|s| (s.url.clone(), s.https_url.clone(), s.body.clone()))
                        .collect();
                    if infos.is_empty() { return None; }
                    Some((pack.id.clone(), pack.name.clone(), infos))
                })
                .collect();

        log!(
            "[sticker] ensure_sticker_pane_loaded: cached={} active_with_stickers={}",
            self.cached_packs.len(), active_packs.len()
        );

        if active_packs.is_empty() {
            self.view.view(cx, ids!(picker_loading)).set_visible(cx, false);
            self.view.view(cx, ids!(picker_scroll)).set_visible(cx, true);
            self.view.redraw(cx);
            return;
        }

        self.sticker_pane_pack_ids = active_packs.iter().map(|(id, _, _)| id.clone()).collect();
        self.show_sticker_pane_loading(cx);
        for (pack_id, pack_name, infos) in active_packs {
            crate::sliding_sync::submit_async_request(
                crate::sliding_sync::MatrixRequest::LoadPackStickers {
                    pack_id,
                    pack_name,
                    sticker_infos: infos,
                },
            );
        }
    }

    // ── Catalog panel state helpers ─────────────────────────────────────────

    fn set_catalog_loading(&mut self, cx: &mut Cx) {
        self.state = ModalState::Loading;
        self.view.view(cx, ids!(loading_state)).set_visible(cx, true);
        self.view.view(cx, ids!(loaded_state)).set_visible(cx, false);
        self.view.view(cx, ids!(error_state)).set_visible(cx, false);
        self.view.view(cx, ids!(sticker_grid_state)).set_visible(cx, false);
        self.view.redraw(cx);
    }

    /// Update catalog panel widgets (may be called while catalog is hidden).
    fn set_catalog_loaded_state(&mut self, cx: &mut Cx, packs: Vec<StickerPack>) {
        self.state = ModalState::Loaded;
        self.view.sticker_pack_list(cx, ids!(pack_list)).set_packs(cx, packs);
        self.view.view(cx, ids!(loading_state)).set_visible(cx, false);
        self.view.view(cx, ids!(loaded_state)).set_visible(cx, true);
        self.view.view(cx, ids!(error_state)).set_visible(cx, false);
        self.view.view(cx, ids!(sticker_grid_state)).set_visible(cx, false);
        self.view.redraw(cx);
    }

    fn set_catalog_error(&mut self, cx: &mut Cx, message: &str) {
        self.state = ModalState::Error;
        self.view.label(cx, ids!(error_label))
            .set_text(cx, &format!("Failed to load sticker packs.\n{}", message));
        self.view.view(cx, ids!(loading_state)).set_visible(cx, false);
        self.view.view(cx, ids!(loaded_state)).set_visible(cx, false);
        self.view.view(cx, ids!(error_state)).set_visible(cx, true);
        self.view.view(cx, ids!(sticker_grid_state)).set_visible(cx, false);
        self.view.redraw(cx);
    }

    /// Return from drill-down back to the catalog list.
    fn show_catalog(&mut self, cx: &mut Cx) {
        self.state = ModalState::Loaded;
        self.view.view(cx, ids!(loading_state)).set_visible(cx, false);
        self.view.view(cx, ids!(loaded_state)).set_visible(cx, true);
        self.view.view(cx, ids!(error_state)).set_visible(cx, false);
        self.view.view(cx, ids!(sticker_grid_state)).set_visible(cx, false);
        self.view.redraw(cx);
    }

    // ── Catalog drill-down (▸ button in pack rows) ──────────────────────────

    fn show_sticker_grid_loading(&mut self, cx: &mut Cx, pack_name: &str) {
        self.state = ModalState::StickerGrid;
        self.current_pack_name = pack_name.to_string();
        self.view.label(cx, ids!(grid_pack_name)).set_text(cx, pack_name);
        self.view.view(cx, ids!(sticker_grid_loading)).set_visible(cx, true);
        self.view.view(cx, ids!(sticker_grid_scroll)).set_visible(cx, false);
        self.view.view(cx, ids!(loading_state)).set_visible(cx, false);
        self.view.view(cx, ids!(loaded_state)).set_visible(cx, false);
        self.view.view(cx, ids!(error_state)).set_visible(cx, false);
        self.view.view(cx, ids!(sticker_grid_state)).set_visible(cx, true);
        self.view.redraw(cx);
    }

    fn set_sticker_grid_loaded(&mut self, cx: &mut Cx, pack_name: &str, stickers: Vec<StickerImage>) {
        self.state = ModalState::StickerGrid;
        self.current_pack_name = pack_name.to_string();
        self.view.label(cx, ids!(grid_pack_name)).set_text(cx, pack_name);
        self.view.sticker_grid(cx, ids!(sticker_grid)).set_stickers(cx, stickers);
        self.view.view(cx, ids!(sticker_grid_loading)).set_visible(cx, false);
        self.view.view(cx, ids!(sticker_grid_scroll)).set_visible(cx, true);
        self.view.view(cx, ids!(loading_state)).set_visible(cx, false);
        self.view.view(cx, ids!(loaded_state)).set_visible(cx, false);
        self.view.view(cx, ids!(error_state)).set_visible(cx, false);
        self.view.view(cx, ids!(sticker_grid_state)).set_visible(cx, true);
        self.view.redraw(cx);
    }
}

impl StickerModalRef {
    /// Reset the modal and kick off a fresh catalog fetch.
    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx);
    }

    /// Open the modal in stickers-only mode (no catalog tab shown).
    pub fn show_stickers_only(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show_stickers_only(cx);
    }
}


// ---------------------------------------------------------------------------
// Async catalog loader (runs inside the matrix worker task)
// ---------------------------------------------------------------------------

/// Walks the scalar/widgets API and returns a list of [`StickerPack`].
///
/// Reads the matrix access token and user id from the active matrix client.
/// The worker task in `sliding_sync.rs` calls this and forwards the result
/// as a [`StickerCatalogAction`].
pub async fn load_sticker_catalog(client: Client) -> Result<Vec<StickerPack>> {
    let user_id = client
        .user_id()
        .ok_or_else(|| anyhow!("matrix client has no user id"))?
        .to_string();
    let matrix_token = client
        .access_token()
        .ok_or_else(|| anyhow!("matrix client has no access token"))?;

    let http = matrix_sdk::reqwest::Client::builder()
        .user_agent("robrix-sticker-client/1.0")
        .pool_idle_timeout(Duration::from_secs(5))
        .tcp_keepalive(Duration::from_secs(30))
        .build()
        .context("building sticker http client")?;

    let cache_path = scalar_token_cache_path();
    let scalar_token = match load_valid_cached_token(&http, &cache_path).await {
        Some(t) => t,
        None => register_and_cache_token(&client, &http, &cache_path, &user_id).await?,
    };

    let widget = request_widget(&http, &scalar_token, WIDGET_TYPE).await?;
    let widget_id = widget
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("widget response missing `id`: {widget}"))?
        .to_string();
    save_cached_widget_id(&cache_path, &widget_id);
    let widget_url = widget
        .get("url")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "https://scalar.vector.im/api/widgets/id/{}/stickers.html",
                widget_id
            )
        });

    if let Err(e) = put_widget_account_data(
        &http,
        &matrix_token,
        &user_id,
        &widget_id,
        WIDGET_TYPE,
        &widget_url,
    )
    .await
    {
        log!("[sticker] PUT account_data/m.widgets failed (continuing): {e}");
    }

    let assets = list_widget_assets(&http, &scalar_token, &widget_id, WIDGET_TYPE).await?;
    log!(
        "[sticker] /api/widgets/assets payload preview: {}",
        truncate_for_log(
            &serde_json::to_string(&assets).unwrap_or_else(|_| "<unserialisable>".into()),
            800,
        )
    );
    let mut packs = parse_packs(&assets);
    log!("[sticker] parsed {} pack(s) from /api/widgets/assets", packs.len());
    for (i, p) in packs.iter().enumerate() {
        log!(
            "[sticker]   pack[{i}] id={} asset_type={} name=\"{}\" active={} stickers={} thumb_url={}",
            p.id, p.asset_type, p.name, p.is_active, p.stickers.len(),
            if p.thumbnail_url.is_empty() { "<none>" } else { p.thumbnail_url.as_str() },
        );
    }
    fetch_thumbnails(&http, &mut packs).await;
    for p in &packs {
        log!(
            "[sticker]   thumb fetch result id={} bytes={}",
            p.id,
            p.thumbnail_bytes.len(),
        );
    }
    Ok(packs)
}

/// Truncate a string for log output, appending a `…(N more chars)` suffix
/// when clipped. Keeps log lines readable on large JSON payloads.
fn truncate_for_log(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    let cut = s.char_indices().nth(max_len).map(|(i, _)| i).unwrap_or(max_len);
    let extra = s.len() - cut;
    format!("{}…({extra} more bytes)", &s[..cut])
}

/// Fetch every pack's thumbnail in parallel and stash the bytes inline.
/// A failed fetch leaves `thumbnail_bytes` empty — the row falls back to
/// just rendering the rounded placeholder background.
async fn fetch_thumbnails(http: &matrix_sdk::reqwest::Client, packs: &mut [StickerPack]) {
    use futures_util::future::join_all;
    log!("[sticker] fetching {} thumbnail(s) in parallel", packs.len());
    let futures = packs.iter().enumerate().map(|(i, p)| {
        let url = p.thumbnail_url.clone();
        let id = p.id.clone();
        async move {
            if url.is_empty() {
                log!("[sticker]   thumb[{id}] skip: empty url");
                return (i, Vec::new());
            }
            log!("[sticker]   thumb[{id}] GET {url}");
            match http.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let status = resp.status();
                    let content_type = resp
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string();
                    match resp.bytes().await {
                        Ok(bytes) => {
                            log!(
                                "[sticker]   thumb[{id}] {status} content-type={content_type} \
                                 bytes={}",
                                bytes.len()
                            );
                            (i, bytes.to_vec())
                        }
                        Err(e) => {
                            log!("[sticker]   thumb[{id}] body read failed: {e}");
                            (i, Vec::new())
                        }
                    }
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    log!(
                        "[sticker]   thumb[{id}] non-success HTTP {status}: {}",
                        truncate_for_log(&body, 200)
                    );
                    (i, Vec::new())
                }
                Err(e) => {
                    log!("[sticker]   thumb[{id}] fetch failed: {e}");
                    (i, Vec::new())
                }
            }
        }
    });
    let results = join_all(futures).await;
    for (i, bytes) in results {
        if let Some(pack) = packs.get_mut(i) {
            pack.thumbnail_bytes = bytes;
        }
    }
}

/// Turn an error into a popup + a `Failed` action in one call.
pub fn report_failure(error: impl std::fmt::Display) {
    let msg = format!("Sticker catalog: {error}");
    log!("[sticker] {msg}");
    enqueue_popup_notification(msg.clone(), PopupKind::Error, Some(5.0));
    Cx::post_action(StickerCatalogAction::Failed { error: msg });
}


// ---------------------------------------------------------------------------
// Sticker catalog disk cache
//
// Stores pack metadata (names, asset types, sticker URLs) so the picker
// overlay can open instantly on the next session without waiting for the
// full scalar API handshake.
//
// A separate image file cache (one .bin per sticker URL hash) ensures that
// sticker PNGs are also served from disk on subsequent openings.
// ---------------------------------------------------------------------------

const CATALOG_CACHE_TTL_SECS: u64 = 24 * 60 * 60; // 24 h

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedPackMeta {
    id: String,
    asset_type: String,
    name: String,
    description: String,
    is_active: bool,
    stickers: Vec<CachedStickerMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedStickerMeta {
    body: String,
    /// The original `mxc://` URL (needed when sending a sticker event).
    #[serde(default)]
    mxc_url: String,
    https_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct StickerCatalogCache {
    cached_at_unix: u64,
    packs: Vec<CachedPackMeta>,
}

fn catalog_cache_path() -> PathBuf {
    let dir = crate::app_data_dir().join("sticker");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log!("[sticker] could not create sticker dir: {e}");
    }
    dir.join("catalog_cache.json")
}

fn sticker_img_cache_dir() -> PathBuf {
    let dir = crate::app_data_dir().join("sticker").join("img_cache");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log!("[sticker] could not create img_cache dir: {e}");
    }
    dir
}

/// Hash a URL into a hex filename for the image cache.
fn url_to_cache_filename(https_url: &str) -> String {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    https_url.hash(&mut h);
    format!("{:016x}.bin", h.finish())
}

fn load_catalog_cache() -> Option<StickerCatalogCache> {
    let path = catalog_cache_path();
    let bytes = std::fs::read(&path).ok()?;
    let cache: StickerCatalogCache = serde_json::from_slice(&bytes).ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    if now.saturating_sub(cache.cached_at_unix) > CATALOG_CACHE_TTL_SECS {
        log!("[sticker] catalog cache expired — ignoring");
        return None;
    }
    Some(cache)
}

fn save_catalog_cache(packs: &[StickerPack]) {
    let path = catalog_cache_path();
    let cached_packs: Vec<CachedPackMeta> = packs.iter().map(|p| CachedPackMeta {
        id: p.id.clone(),
        asset_type: p.asset_type.clone(),
        name: p.name.clone(),
        description: p.description.clone(),
        is_active: p.is_active,
        stickers: p.stickers.iter().map(|s| CachedStickerMeta {
            body: s.body.clone(),
            mxc_url: s.url.clone(),
            https_url: s.https_url.clone(),
        }).collect(),
    }).collect();

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let cache = StickerCatalogCache { cached_at_unix: now, packs: cached_packs };
    match serde_json::to_vec_pretty(&cache) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                log!("[sticker] could not write catalog cache: {e}");
            } else {
                log!("[sticker] catalog cache saved ({} pack(s))", packs.len());
            }
        }
        Err(e) => log!("[sticker] could not encode catalog cache: {e}"),
    }
}

fn scalar_token_cache_path() -> PathBuf {
    let dir = crate::app_data_dir().join("sticker");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log!("[sticker] could not create cache dir {}: {e}", dir.display());
    }
    dir.join("scalar_token.json")
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CachedScalarToken {
    scalar_token: String,
    created_at_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    /// The stickerpicker widget id returned by `/api/widgets/request`.
    /// Cached so that toggle calls can avoid re-running the full request
    /// flow on every click.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    widget_id: Option<String>,
}

fn load_cached_token(path: &Path) -> Option<CachedScalarToken> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn save_cached_token(path: &Path, token: &str, user_id: &str) -> Result<()> {
    let created_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before unix epoch")?
        .as_secs();
    // Preserve any previously-cached widget_id so we don't lose it when
    // the token is rotated.
    let existing_widget_id = load_cached_token(path).and_then(|e| e.widget_id);
    let entry = CachedScalarToken {
        scalar_token: token.to_string(),
        created_at_unix,
        user_id: Some(user_id.to_string()),
        widget_id: existing_widget_id,
    };
    let json = serde_json::to_vec_pretty(&entry)?;
    std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Update only the cached `widget_id`, leaving the token and other fields
/// untouched. Best-effort: any I/O error is logged and ignored.
fn save_cached_widget_id(path: &Path, widget_id: &str) {
    let Some(mut entry) = load_cached_token(path) else {
        log!("[sticker] cannot stash widget_id: no cached token at {}", path.display());
        return;
    };
    entry.widget_id = Some(widget_id.to_string());
    match serde_json::to_vec_pretty(&entry) {
        Ok(json) => {
            if let Err(e) = std::fs::write(path, json) {
                log!("[sticker] could not write widget_id to {}: {e}", path.display());
            }
        }
        Err(e) => log!("[sticker] could not encode widget_id cache: {e}"),
    }
}


async fn load_valid_cached_token(
    http: &matrix_sdk::reqwest::Client,
    cache_path: &Path,
) -> Option<String> {
    let entry = load_cached_token(cache_path)?;
    match validate_scalar_token(http, &entry.scalar_token).await {
        Ok(Some(_user_id)) => Some(entry.scalar_token),
        Ok(None) => {
            let _ = std::fs::remove_file(cache_path);
            None
        }
        Err(e) => {
            log!("[sticker] scalar token validation errored: {e}; re-registering");
            None
        }
    }
}

async fn register_and_cache_token(
    client: &Client,
    http: &matrix_sdk::reqwest::Client,
    cache_path: &Path,
    user_id: &str,
) -> Result<String> {
    let openid = fetch_openid_token(client, user_id).await?;
    let scalar_token = register_with_scalar(http, &openid).await?;
    if let Err(e) = save_cached_token(cache_path, &scalar_token, user_id) {
        log!("[sticker] could not cache scalar_token: {e}");
    }
    Ok(scalar_token)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct OpenIdToken {
    access_token: String,
    token_type: String,
    matrix_server_name: String,
    #[serde(default)]
    expires_in: u64,
}

/// Use the matrix-sdk's account API rather than building a raw HTTP request —
/// the SDK already knows the homeserver URL and access token, so we don't have
/// to plumb either through manually.
async fn fetch_openid_token(client: &Client, user_id: &str) -> Result<OpenIdToken> {
    use matrix_sdk::ruma::{
        UserId,
        api::client::account::request_openid_token::v3::Request as OpenIdRequest,
    };
    let uid = <&UserId>::try_from(user_id)
        .map_err(|e| anyhow!("invalid user id {user_id}: {e}"))?;
    let resp = client
        .send(OpenIdRequest::new(uid.to_owned()))
        .await
        .context("matrix /openid/request_token")?;
    Ok(OpenIdToken {
        access_token: resp.access_token,
        token_type: resp.token_type.to_string(),
        matrix_server_name: resp.matrix_server_name.to_string(),
        expires_in: resp.expires_in.as_secs(),
    })
}

async fn validate_scalar_token(
    http: &matrix_sdk::reqwest::Client,
    scalar_token: &str,
) -> Result<Option<String>> {
    let resp = http
        .get(SCALAR_ACCOUNT_URL)
        .bearer_auth(scalar_token)
        .header("Accept", "*/*")
        .header("Origin", "https://scalar.vector.im")
        .send()
        .await
        .context("scalar /api/account: network")?;
    let status = resp.status();
    if status == matrix_sdk::reqwest::StatusCode::UNAUTHORIZED
        || status == matrix_sdk::reqwest::StatusCode::FORBIDDEN
    {
        return Ok(None);
    }
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!("scalar /api/account: HTTP {status} body={body}"));
    }
    #[derive(Deserialize)]
    struct AccountResp { user_id: String }
    let parsed: AccountResp = serde_json::from_str(&body)
        .with_context(|| format!("scalar /api/account: bad JSON body={body}"))?;
    Ok(Some(parsed.user_id))
}

async fn register_with_scalar(
    http: &matrix_sdk::reqwest::Client,
    openid: &OpenIdToken,
) -> Result<String> {
    let body = serde_json::to_vec(openid).context("encoding openid body")?;
    let resp = http
        .post(SCALAR_REGISTER_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "*/*")
        .header("Origin", "https://app.element.io")
        .body(body)
        .send()
        .await
        .context("scalar /api/register: network")?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!("scalar /api/register: HTTP {status} body={text}"));
    }
    #[derive(Deserialize)]
    struct Resp { scalar_token: String }
    let parsed: Resp = serde_json::from_str(&text)
        .with_context(|| format!("scalar /api/register: bad JSON body={text}"))?;
    Ok(parsed.scalar_token)
}

async fn put_widget_account_data(
    http: &matrix_sdk::reqwest::Client,
    matrix_token: &str,
    user_id: &str,
    widget_id: &str,
    widget_type: &str,
    widget_url: &str,
) -> Result<()> {
    let url = format!(
        "https://matrix-client.matrix.org/_matrix/client/v3/user/{}/account_data/m.widgets",
        percent_encoding::utf8_percent_encode(user_id, percent_encoding::NON_ALPHANUMERIC),
    );
    let body = serde_json::json!({
        widget_id: {
            "content": {
                "id": widget_id,
                "type": widget_type,
                "url": widget_url,
                "data": {},
                "creatorUserId": user_id,
            },
            "sender": user_id,
            "state_key": widget_id,
            "type": "m.widget",
            "id": widget_id,
        }
    });
    let encoded = serde_json::to_vec(&body)?;
    let resp = http
        .put(&url)
        .bearer_auth(matrix_token)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(encoded)
        .send()
        .await
        .context("matrix PUT account_data/m.widgets: network")?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!(
            "matrix PUT account_data/m.widgets: HTTP {status} body={text}"
        ));
    }
    Ok(())
}

async fn list_widget_assets(
    http: &matrix_sdk::reqwest::Client,
    scalar_token: &str,
    widget_id: &str,
    widget_type: &str,
) -> Result<serde_json::Value> {
    // matrix_sdk::reqwest doesn't re-export RequestBuilder::query, so we
    // build the query string ourselves.
    let url = format!(
        "{base}?widget_id={wid}&widget_type={wtype}",
        base = SCALAR_WIDGETS_ASSETS_URL,
        wid = percent_encoding::utf8_percent_encode(
            widget_id, percent_encoding::NON_ALPHANUMERIC),
        wtype = percent_encoding::utf8_percent_encode(
            widget_type, percent_encoding::NON_ALPHANUMERIC),
    );
    let resp = http
        .get(&url)
        .bearer_auth(scalar_token)
        .header("Accept", "*/*")
        .header("Origin", "https://scalar.vector.im")
        .header("Pragma", "no-cache")
        .header("Cache-Control", "no-cache")
        .send()
        .await
        .context("scalar /api/widgets/assets: network")?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    log!("[sticker] GET {url} → HTTP {status} body_len={}\n[sticker] FULL BODY: {body}", body.len());
    if !status.is_success() {
        return Err(anyhow!("scalar /api/widgets/assets: HTTP {status} body={body}"));
    }
    serde_json::from_str(&body)
        .with_context(|| format!("scalar /api/widgets/assets: bad JSON body={body}"))
}

async fn request_widget(
    http: &matrix_sdk::reqwest::Client,
    scalar_token: &str,
    widget_type: &str,
) -> Result<serde_json::Value> {
    let body = serde_json::to_vec(&serde_json::json!({ "type": widget_type }))?;
    let resp = http
        .post(SCALAR_WIDGETS_REQUEST_URL)
        .bearer_auth(scalar_token)
        .header("Content-Type", "application/json")
        .header("Accept", "*/*")
        .header("Origin", "https://scalar.vector.im")
        .body(body)
        .send()
        .await
        .context("scalar /api/widgets/request: network")?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status == matrix_sdk::reqwest::StatusCode::CONFLICT {
        if let Some(existing) = parse_existing_widget_from_conflict(&text, widget_type) {
            return Ok(existing);
        }
        return Err(anyhow!(
            "scalar /api/widgets/request: HTTP 409 body={text} (no usable data.id)"
        ));
    }
    if !status.is_success() {
        return Err(anyhow!("scalar /api/widgets/request: HTTP {status} body={text}"));
    }
    serde_json::from_str(&text)
        .with_context(|| format!("scalar /api/widgets/request: bad JSON body={text}"))
}

fn parse_existing_widget_from_conflict(
    body: &str,
    widget_type: &str,
) -> Option<serde_json::Value> {
    let parsed: serde_json::Value = serde_json::from_str(body).ok()?;
    let data = parsed.get("data")?;
    let id = data.get("id")?.as_str()?;
    let url = data
        .get("url")
        .and_then(|v| v.as_str())
        .or_else(|| data.get("wurl").and_then(|v| v.as_str()))
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!("https://scalar.vector.im/api/widgets/id/{}/stickers.html", id)
        });
    Some(serde_json::json!({
        "id": id,
        "url": url,
        "type": widget_type,
    }))
}

/// Parses `/api/widgets/assets` into our row model. The response is a
/// top-level `AssetCollection`:
///
/// ```text
/// { "id": "...", "assets": [ Asset { name, description, thumbnail,
///                                    purchased, data: { thumbnail,
///                                    image_path, ... } } ] }
/// ```
///
/// Some envelopes (older deployments, errors) wrap it under `data` — we try
/// both, then walk the `assets` array. Within an asset, prefer the top-level
/// `name/description/thumbnail` and fall back to `data.thumbnail` /
/// `data.image_path` when the top-level field is empty.
fn parse_packs(assets: &serde_json::Value) -> Vec<StickerPack> {
    // Find the collection root (may be the assets value itself or under `data`).
    let collection_root = assets
        .get("assets")
        .map(|_| assets)
        .or_else(|| assets.get("data"))
        .unwrap_or(assets);

    let collection_id = collection_root
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Walk `assets` (the documented shape) or a few legacy fallbacks.
    let arr = collection_root
        .get("assets")
        .and_then(|v| v.as_array())
        .or_else(|| collection_root.get("packs").and_then(|v| v.as_array()))
        .or_else(|| collection_root.as_array());
    let Some(arr) = arr else { return Vec::new() };

    arr.iter()
        .enumerate()
        .map(|(i, item)| {
            // Inner `data` overrides for fields that may only live there.
            let inner = item.get("data");
            let read_str = |key: &str| -> String {
                let top = item.get(key).and_then(|v| v.as_str()).unwrap_or("");
                if !top.is_empty() {
                    return top.to_string();
                }
                inner
                    .and_then(|d| d.get(key))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let mut name = read_str("name");
            let mut description = read_str("description");
            let mut thumbnail = read_str("thumbnail");
            // Fall back to `data.image_path` when no thumbnail is set.
            if thumbnail.is_empty() {
                thumbnail = inner
                    .and_then(|d| d.get("image_path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            // `asset_type` is the wire identity (e.g. "isabella"). Prefer
            // the top-level field; fall back to legacy `id`.
            let asset_type = item
                .get("asset_type")
                .and_then(|v| v.as_str())
                .or_else(|| item.get("id").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();
            let id = if !collection_id.is_empty() && !asset_type.is_empty() {
                format!("{collection_id}_{asset_type}")
            } else if !asset_type.is_empty() {
                asset_type.clone()
            } else {
                format!("pack_{i}")
            };
            if name.is_empty() {
                name = if !asset_type.is_empty() { asset_type.clone() } else { id.clone() };
            }
            if description.is_empty() {
                description = collection_id.clone();
            }
            // The new schema uses `purchased`; the previous one used
            // `is_active`. Accept either.
            let is_active = item
                .get("purchased")
                .or_else(|| item.get("is_active"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Extract individual stickers from `data.images[].content.{body,url}`.
            // The API returns purchased packs with data.images[]; non-purchased packs have no images.
            let sticker_arr = inner
                .and_then(|d| d.get("images"))
                .and_then(|s| s.as_array());
            let stickers: Vec<StickerImage> = sticker_arr
                .map(|arr| {
                    arr.iter().filter_map(|s| {
                        let content = s.get("content")?;
                        let body = content.get("body")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let raw_url = content.get("url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if raw_url.is_empty() { return None; }
                        let https_url = mxc_to_https(&raw_url);
                        Some(StickerImage {
                            body,
                            url: raw_url,
                            https_url,
                            width: 0,
                            height: 0,
                            image_bytes: Vec::new(),
                        })
                    }).collect()
                })
                .unwrap_or_default();

            StickerPack {
                id,
                asset_type,
                name,
                description,
                thumbnail_url: absolutize_thumbnail_url(&thumbnail),
                thumbnail_bytes: Vec::new(),
                is_active,
                stickers,
            }
        })
        .collect()
}

/// Progressive sticker loader.  Called by the matrix worker for every
/// `MatrixRequest::LoadPackStickers`.
///
/// **Two-phase strategy:**
/// 1. Synchronously check the disk cache for every sticker.  Post
///    `StickerGridAction::Ready` immediately so the UI can render all
///    cached tiles (instant on repeat opens) and show placeholders for
///    anything not yet on disk — no spinner needed.
/// 2. Fetch the remaining cache-miss images concurrently.  Post a
///    `StickerImagePatchAction` every time a batch of 4 completes so tiles
///    fill in progressively rather than all at once after a long wait.
pub async fn load_pack_stickers_streaming(
    pack_id: String,
    pack_name: String,
    sticker_infos: Vec<(String, String, String)>,
) {
    log!(
        "[sticker] load_pack_stickers_streaming: pack={pack_name:?} n={}",
        sticker_infos.len()
    );

    // ── Phase 1: synchronous disk-cache pass ──────────────────────────────
    let stickers: Vec<StickerImage> = sticker_infos
        .iter()
        .map(|(mxc_url, https_url, body)| {
            let image_bytes = if !https_url.is_empty() {
                let path = sticker_img_cache_dir()
                    .join(url_to_cache_filename(https_url));
                std::fs::read(&path).unwrap_or_default()
            } else {
                Vec::new()
            };
            let cached = !image_bytes.is_empty();
            log!(
                "[sticker]   disk {} body={body:?}",
                if cached { "HIT " } else { "MISS" }
            );
            let (width, height) = parse_png_dimensions(&image_bytes);
            StickerImage {
                url: mxc_url.clone(),
                https_url: https_url.clone(),
                body: body.clone(),
                width,
                height,
                image_bytes,
            }
        })
        .collect();

    // Post immediately — all cached tiles show at once; placeholders appear
    // for anything that still needs a network fetch.
    Cx::post_action(StickerGridAction::Ready {
        pack_id: pack_id.clone(),
        pack_name: pack_name.clone(),
        stickers: stickers.clone(),
    });

    // ── Phase 2: network fetch for cache misses ───────────────────────────
    let missing: Vec<(usize, String)> = stickers
        .iter()
        .enumerate()
        .filter(|(_, s)| s.image_bytes.is_empty() && !s.https_url.is_empty())
        .map(|(i, s)| (i, s.https_url.clone()))
        .collect();

    if missing.is_empty() {
        log!("[sticker] all sticker images served from disk cache — done");
        return;
    }

    log!("[sticker] {} cache-miss sticker(s) to fetch", missing.len());

    let http = match matrix_sdk::reqwest::Client::builder()
        .user_agent("robrix-sticker-client/1.0")
        .pool_idle_timeout(Duration::from_secs(5))
        .tcp_keepalive(Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            log!("[sticker] could not build http client for patch fetches: {e}");
            return;
        }
    };

    // Use FuturesUnordered so each image posts a patch as soon as it arrives
    // (in batches of PATCH_BATCH_SIZE to avoid hammering the UI with redraws).
    use futures_util::stream::FuturesUnordered;
    use futures_util::StreamExt as _;

    const PATCH_BATCH_SIZE: usize = 4;

    // reqwest::Client is a cheap Arc clone — each future gets its own handle.
    let mut pending: FuturesUnordered<_> = missing
        .into_iter()
        .map(|(idx, url)| {
            let http = http.clone();
            async move {
                let bytes = fetch_cached_sticker_image(&http, &url).await;
                (idx, bytes)
            }
        })
        .collect();

    let mut batch: Vec<(usize, Vec<u8>)> = Vec::with_capacity(PATCH_BATCH_SIZE);

    while let Some((idx, bytes)) = pending.next().await {
        if !bytes.is_empty() {
            batch.push((idx, bytes));
            if batch.len() >= PATCH_BATCH_SIZE {
                Cx::post_action(StickerImagePatchAction {
                    pack_id: pack_id.clone(),
                    updates: std::mem::take(&mut batch),
                });
            }
        }
    }
    if !batch.is_empty() {
        Cx::post_action(StickerImagePatchAction {
            pack_id,
            updates: batch,
        });
    }
}

/// Fetch one sticker image — from the disk cache when available, otherwise
/// from the network.  Successful network fetches are stored to disk.
async fn fetch_cached_sticker_image(
    http: &matrix_sdk::reqwest::Client,
    https_url: &str,
) -> Vec<u8> {
    if https_url.is_empty() {
        return Vec::new();
    }
    let cache_file = sticker_img_cache_dir().join(url_to_cache_filename(https_url));
    if let Ok(bytes) = std::fs::read(&cache_file) {
        if !bytes.is_empty() {
            log!("[sticker] img_cache HIT {} bytes ← {https_url}", bytes.len());
            return bytes;
        }
    }
    log!("[sticker] img_cache MISS → GET {https_url}");
    match http.get(https_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let status = resp.status();
            match resp.bytes().await {
                Ok(bytes) => {
                    log!("[sticker]   {status} {} bytes for {https_url}", bytes.len());
                    if let Err(e) = std::fs::write(&cache_file, &bytes) {
                        log!("[sticker]   could not write img cache: {e}");
                    }
                    bytes.to_vec()
                }
                Err(e) => { log!("[sticker]   body read failed: {e}"); Vec::new() }
            }
        }
        Ok(resp) => { log!("[sticker]   HTTP {}", resp.status()); Vec::new() }
        Err(e)   => { log!("[sticker]   fetch failed: {e}"); Vec::new() }
    }
}

/// Push a pack's active/inactive state to scalar.
///
/// Behaviour mirrors what the Element web client does on each toggle:
///   * **Enable**  → re-fetch the catalog via `/api/widgets/assets`.
///     The server side-effects "active" out of the listing call.
///   * **Disable** → `GET /api/widgets/set_asset_state?...&state=disable`.
///
/// Uses the cached scalar token + widget id (`load_sticker_catalog` is
/// responsible for populating them). If either is missing we silently fall
/// back to a full catalog reload, which is what would have rebuilt the
/// cache anyway.
pub async fn set_pack_state(client: Client, asset_type: String, enable: bool) -> Result<()> {
    log!(
        "[sticker] set_pack_state: asset_type={asset_type} enable={enable} \
         (enable→/api/widgets/assets, disable→/api/widgets/set_asset_state)"
    );
    let cache_path = scalar_token_cache_path();
    let Some(entry) = load_cached_token(&cache_path) else {
        log!(
            "[sticker] set_pack_state: no cached scalar session at {} — \
             running full catalog bootstrap before retrying",
            cache_path.display(),
        );
        let _ = load_sticker_catalog(client).await?;
        return Ok(());
    };
    let Some(widget_id) = entry.widget_id.as_deref() else {
        log!(
            "[sticker] set_pack_state: cached token has no widget_id — \
             running full catalog bootstrap before retrying"
        );
        let _ = load_sticker_catalog(client).await?;
        return Ok(());
    };
    let scalar_token = entry.scalar_token;
    let http = matrix_sdk::reqwest::Client::builder()
        .user_agent("robrix-sticker-client/1.0")
        .build()
        .context("building sticker http client")?;
    if enable {
        let url = format!(
            "{base}?widget_id={wid}&widget_type={wtype}&asset_type={atype}",
            base = SCALAR_WIDGETS_PURCHASE_ASSET_URL,
            wid = percent_encoding::utf8_percent_encode(
                widget_id, percent_encoding::NON_ALPHANUMERIC),
            wtype = percent_encoding::utf8_percent_encode(
                WIDGET_TYPE, percent_encoding::NON_ALPHANUMERIC),
            atype = percent_encoding::utf8_percent_encode(
                &asset_type, percent_encoding::NON_ALPHANUMERIC),
        );
        log!("[sticker] purchase_asset GET {url}");
        let resp = http
            .get(&url)
            .bearer_auth(&scalar_token)
            .header("Accept", "*/*")
            .header("Origin", "https://scalar.vector.im")
            .header("Referer", "https://scalar.vector.im/")
            .send()
            .await
            .context("scalar /api/widgets/purchase_asset: network")?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            log!("[sticker] purchase_asset FAILED asset_type={asset_type} HTTP {status} body={body}");
            return Err(anyhow!("scalar /api/widgets/purchase_asset: HTTP {status} body={body}"));
        }
        log!("[sticker] purchase_asset OK asset_type={asset_type} HTTP {status} body={body}");
    } else {
        let url = format!(
            "{base}?widget_id={wid}&widget_type={wtype}&asset_type={atype}&state=disable",
            base = SCALAR_WIDGETS_SET_STATE_URL,
            wid = percent_encoding::utf8_percent_encode(
                widget_id, percent_encoding::NON_ALPHANUMERIC),
            wtype = percent_encoding::utf8_percent_encode(
                WIDGET_TYPE, percent_encoding::NON_ALPHANUMERIC),
            atype = percent_encoding::utf8_percent_encode(
                &asset_type, percent_encoding::NON_ALPHANUMERIC),
        );
        log!("[sticker] set_asset_state GET {url} (state=disable)");
        let resp = http
            .get(&url)
            .bearer_auth(&scalar_token)
            .header("Accept", "*/*")
            .header("Origin", "https://scalar.vector.im")
            .header("Referer", "https://scalar.vector.im/")
            .send()
            .await
            .context("scalar /api/widgets/set_asset_state: network")?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            log!("[sticker] set_asset_state FAILED asset_type={asset_type} HTTP {status} body={body}");
            return Err(anyhow!("scalar /api/widgets/set_asset_state: HTTP {status} body={body}"));
        }
        log!("[sticker] set_asset_state OK asset_type={asset_type} HTTP {status} body={body}");
    }
    Ok(())
}

/// Convert a `mxc://server/media_id` URI into the HTTPS download URL that
/// a Matrix media repository will serve.  Non-mxc URLs are returned as-is.
/// Returns an empty string when conversion is not possible.
/// Extract (width, height) from the first 24 bytes of a PNG file.
/// Returns (0, 0) if the bytes are not a valid PNG or are too short.
fn parse_png_dimensions(bytes: &[u8]) -> (u32, u32) {
    const PNG_SIG: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[0..8] != PNG_SIG || &bytes[12..16] != b"IHDR" {
        return (0, 0);
    }
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    (w, h)
}

fn mxc_to_https(url: &str) -> String {
    let url = url.trim();
    if url.is_empty() {
        return String::new();
    }
    if url.starts_with("https://") || url.starts_with("http://") {
        return url.to_string();
    }
    if let Some(rest) = url.strip_prefix("mxc://") {
        if let Some(slash) = rest.find('/') {
            let server = &rest[..slash];
            let media_id = &rest[slash + 1..];
            return format!(
                "https://{}/_matrix/media/v3/download/{}/{}",
                server, server, media_id,
            );
        }
    }
    String::new()
}

/// Scalar's `thumbnail` field is sometimes a relative path. Resolve it
/// against the integrations server when it doesn't already carry a scheme.
fn absolutize_thumbnail_url(thumbnail: &str) -> String {
    let s = thumbnail.trim();
    if s.is_empty() {
        return String::new();
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        return s.to_string();
    }
    if let Some(rest) = s.strip_prefix("//") {
        return format!("https://{rest}");
    }
    let suffix = s.trim_start_matches('/');
    format!("https://scalar.vector.im/{suffix}")
}
