//! The `WelcomeScreen` shown in the desktop Home tab, with a few tips for new users.

use makepad_widgets::*;
use matrix_sdk::{RoomDisplayName, RoomState, ruma::{OwnedRoomId, owned_room_id}};
use crate::{app::AppStateAction, home::{invite_screen::LeaveRoomResultAction, rooms_list::RoomsListRef}, logout::logout_confirm_modal::LogoutAction, room::BasicRoomDetails, utils::RoomNameId};

/// Below this width, the Robrix room panel centers its text and button.
const ROBRIX_ROOM_PANEL_CENTERING_WIDTH: f64 = 405.0;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.WELCOME_TEXT_MUTED = #5C6478

    // A tip in the welcome screen's grid, which fits three tips per row
    // if the grid is at least 675px wide, otherwise one per row.
    mod.widgets.WelcomeTip = View {
        width: "max(33.33% - 17px, min(100%, (675px - 100%) * 10000))"
        height: Fit
        flow: Down, spacing: 14

        upper := View {
            width: Fill, height: Fit
            flow: Down, spacing: 10

            header := View {
                width: Fill, height: Fit
                flow: Right, spacing: 12
                align: Align{y: 0.5}

                badge := RoundedView {
                    width: 44, height: 44
                    align: Align{x: 0.5, y: 0.5}
                    draw_bg +: { border_radius: 12.0 }
                    icon := Icon {
                        icon_walk: Walk{width: 22, height: 22}
                    }
                }
                title := Label {
                    width: Fill, padding: 0
                    draw_text +: {
                        color: (COLOR_TEXT)
                        text_style: theme.font_bold { font_size: 13.5 }
                    }
                }
            }

            // Lines up the body's text with the hint's text below.
            body_view := View {
                width: Fill, height: Fit
                padding: Inset{left: 10}
                body := Label {
                    width: Fill, padding: 0
                    draw_text +: {
                        color: (mod.widgets.WELCOME_TEXT_MUTED)
                        text_style: theme.font_regular { font_size: 12, line_spacing: 1.4 }
                    }
                }
            }
        }

        hint := RoundedView {
            width: Fill, height: Fit
            padding: Inset{left: 10, right: 10, top: 8, bottom: 8}
            draw_bg +: { border_radius: 8.0 }
            hint_label := Label {
                width: Fill, padding: 0
                draw_text +: {
                    text_style: theme.font_regular { font_size: 11.5, line_spacing: 1.35 }
                }
            }
        }
    }

    // A thin divider between the tips (only when they're vertically stacked).
    mod.widgets.WelcomeTipDivider = SolidView {
        visible: false
        width: "100% - 48px", height: 1
        margin: Inset{left: 24, right: 24}
        draw_bg.color: (COLOR_DIVIDER)
    }

    mod.widgets.WelcomeScreen = set_type_default() do #(WelcomeScreen::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill, height: Fill
        flow: Down
        align: Align{x: 0.5, y: 0.5}
        padding: Inset{left: 24, right: 24}
        cursor: MouseCursor.Default

        show_bg: true
        draw_bg +: {
            color: #F3EFFD
            color_2: (COLOR_PRIMARY)
            border_radius: 0.0
        }

        // make this view behave like a ScrollYView
        scroll_bars: mod.widgets.ScrollBars {
            show_scroll_x: false show_scroll_y: true
            scroll_bar_y.drag_scrolling: true
        }

        content := View {
            width: Fill{max: 900}, height: Fit
            flow: Down, spacing: 24
            padding: Inset{top: 32, bottom: 32}
            // for shadows
            clip_x: false, clip_y: false

            title_view := View {
                width: Fill, height: Fit
                flow: Down, spacing: 10
                align: Align{x: 0.5}

                Image {
                    width: 72, height: 72
                    fit: ImageFit.Smallest
                    src: (mod.widgets.IMG_APP_LOGO)
                }
                Label {
                    width: Fill, padding: 0
                    align: Align{x: 0.5}
                    text: "Welcome to Robrix!"
                    draw_text +: {
                        color: (COLOR_TEXT)
                        text_style: theme.font_bold { font_size: 28 }
                    }
                }
            }

            robrix_room_panel := RoundedShadowView {
                width: Fill, height: Fit
                flow: Down, spacing: 12
                padding: Inset{left: 24, right: 24, top: 22, bottom: 22}
                draw_bg +: {
                    color: (COLOR_ROBRIX_PURPLE)
                    color_2: #1F7BF2
                    gradient_fill_horizontal: 1.0
                    border_radius: 16.0
                    shadow_color: #x572DCC40
                    shadow_radius: 18.0
                    shadow_offset: vec2(0.0, 6.0)
                }

                robrix_room_header := View {
                    width: Fill, height: Fit
                    flow: Right, spacing: 16
                    align: Align{y: 0.5}

                    RoundedView {
                        width: 52, height: 52
                        align: Align{x: 0.5, y: 0.5}
                        draw_bg +: { color: #xFFFFFF2E, border_radius: 14.0 }
                        Icon {
                            icon_walk: Walk{width: 26, height: 26}
                            draw_icon +: { svg: (ICON_REPLY_IN_THREAD), color: #fff }
                        }
                    }
                    Label {
                        width: Fill, padding: 0
                        text: "Check out our Matrix room"
                        draw_text +: {
                            color: #fff
                            text_style: theme.font_bold { font_size: 17 }
                        }
                    }
                }

                View {
                    width: Fill, height: Fit
                    flow: Right, spacing: 16

                    robrix_room_details_indent := View { width: 52, height: 0 }
                    robrix_room_details := View {
                        width: Fill, height: Fit
                        flow: Down, spacing: 18

                        robrix_room_description := Label {
                            width: Fill, padding: 0
                            text: "Feel free to ask questions, share feedback, or catch up on the latest announcements."
                            draw_text +: {
                                color: #xFFFFFFD9
                                text_style: theme.font_regular { font_size: 13, line_spacing: 1.35 }
                            }
                        }

                        join_robrix_room_button := RobrixIconButton {
                            padding: Inset{left: 14, right: 16, top: 10, bottom: 10}
                            spacing: 8
                            draw_bg +: {
                                color: #fff
                                color_hover: (COLOR_BG_LAVENDER_HOVER)
                                color_down: (COLOR_BG_LAVENDER_DOWN)
                                border_radius: 8.0
                            }

                            draw_icon +: { svg: (ICON_JOIN_ROOM), color: (COLOR_ROBRIX_PURPLE) }
                            draw_text +: {
                                color: (COLOR_ROBRIX_PURPLE)
                                color_hover: (COLOR_ROBRIX_PURPLE)
                                color_down: (COLOR_ROBRIX_PURPLE)
                                text_style: theme.font_bold { font_size: 11 }
                            }
                            text: "Join the Robrix room"
                        }
                    }
                }
            }

            tips_panel := RoundedShadowView {
                width: Fill, height: Fit
                flow: Down, spacing: 20
                padding: 26
                draw_bg +: {
                    color: #fff
                    border_radius: 16.0
                    border_size: 1.0
                    border_color: #x00000010
                    shadow_color: #x1C274C14
                    shadow_radius: 16.0
                    shadow_offset: vec2(0.0, 4.0)
                }

                Label {
                    width: Fill, padding: 0
                    text: "Here are some tips to get started"
                    draw_text +: {
                        color: (COLOR_TEXT)
                        text_style: theme.font_bold { font_size: 16 }
                    }
                }

                tips_grid := View {
                    width: Fill, height: Fit
                    flow: Flow.Right{wrap: true}
                    spacing: 24, wrap_spacing: 20

                    workspace_tip := mod.widgets.WelcomeTip {
                        upper +: {
                            header +: {
                                badge +: {
                                    draw_bg.color: (COLOR_BG_LAVENDER)
                                    icon +: { draw_icon +: { svg: (ICON_SQUARES), color: (COLOR_ROBRIX_PURPLE) } }
                                }
                                title +: { text: "Arrange your workspace" }
                            }
                            body_view +: { body +: { text: "Open rooms from the rooms list on the left, then drag & drop tabs to rearrange them into multiple panes, side by side!" } }
                        }
                        hint +: {
                            draw_bg.color: #xF4F1FE
                            hint_label +: {
                                text: "There's no limits on dockable tabs, so go wild!"
                                draw_text.color: #x4A2BA8
                            }
                        }
                    }
                    tip_divider_1 := mod.widgets.WelcomeTipDivider {}
                    settings_tip := mod.widgets.WelcomeTip {
                        upper +: {
                            header +: {
                                badge +: {
                                    draw_bg.color: #xE3F0FF
                                    icon +: { draw_icon +: { svg: (ICON_SETTINGS), color: (COLOR_ACTIVE_PRIMARY) } }
                                }
                                title +: { text: "Explore more options" }
                            }
                            body_view +: { body +: { text: "Tap/click your profile icon to open all app and user settings." } }
                        }
                        hint +: {
                            draw_bg.color: #xEEF6FF
                            hint_label +: {
                                text: "Try zooming the UI! It's super fast and fine-tuned; keyboard shortcuts work too."
                                draw_text.color: #x0B5CAB
                            }
                        }
                    }
                    tip_divider_2 := mod.widgets.WelcomeTipDivider {}
                    navigation_tip := mod.widgets.WelcomeTip {
                        upper +: {
                            header +: {
                                badge +: {
                                    draw_bg.color: #xDDF6F5
                                    icon +: { draw_icon +: { svg: (ICON_JUMP), color: #x039E99 } }
                                }
                                title +: { text: "Use mouse buttons" }
                            }
                            body_view +: { body +: { text: "Just like a browser: navigate backwards and forwards through your history of visited rooms, or middle-click to close a tab." } }
                        }
                        hint +: {
                            draw_bg.color: #xEAF9F8
                            hint_label +: {
                                text: "Full keyboard navigation and custom shortcuts are coming soon!"
                                draw_text.color: #x02706C
                            }
                        }
                    }
                }
            }

            footer := View {
                width: Fill, height: Fit
                flow: Down, spacing: 6
                align: Align{x: 0.5}
                padding: Inset{top: 8, left: 8, right: 8}

                Label {
                    width: Fill, padding: 0
                    align: Align{x: 0.5}
                    text: "Robrix is still being actively developed. You might need another client to do certain administrative actions for now."
                    draw_text +: {
                        color: #x6B7287
                        text_style: theme.font_regular { font_size: 12, line_spacing: 1.35 }
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct WelcomeScreen {
    #[deref] view: View,

    #[rust(owned_room_id!("!moVNEIUPxJZpxRHDUv:matrix.org"))] robrix_room_id: OwnedRoomId,

    /// Whether the user has joined the Robrix room, or `None` if we haven't checked yet.
    #[rust] is_robrix_room_joined: Option<bool>,
}

impl Widget for WelcomeScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let Event::Actions(actions) = event else { return };

        if self.view.button(cx, ids!(join_robrix_room_button)).clicked(actions) {
            cx.action(AppStateAction::NavigateToRoom {
                room_to_close: None,
                destination_room: BasicRoomDetails::Name(RoomNameId::new(
                    RoomDisplayName::Named(String::from("Robrix")),
                    self.robrix_room_id.clone(),
                )),
            });
        }

        for action in actions {
            if let Some(AppStateAction::RoomLoadedSuccessfully { room_name_id, is_invite: false }) = action.downcast_ref()
                && room_name_id.room_id() == &self.robrix_room_id
            {
                self.set_robrix_room_joined(cx, true);
            }
            if let Some(LeaveRoomResultAction::Left { room_id }) = action.downcast_ref()
                && room_id == &self.robrix_room_id
            {
                self.set_robrix_room_joined(cx, false);
            }
            if let Some(LogoutAction::ClearAppState { .. }) = action.downcast_ref() {
                self.is_robrix_room_joined = None;
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.is_robrix_room_joined.is_none() {
            let room_state = cx.get_global::<RoomsListRef>().get_room_state(&self.robrix_room_id);
            self.set_robrix_room_joined(cx, matches!(room_state, Some(RoomState::Joined)));
        }
        let step = self.view.draw_walk(cx, scope, walk);
        self.update_tips_layout(cx);
        self.update_robrix_room_panel_layout(cx);
        step
    }
}

impl WelcomeScreen {
    /// If the Robrix room is joined already, show "Go to" instead of "Join".
    fn set_robrix_room_joined(&mut self, cx: &mut Cx, is_joined: bool) {
        self.is_robrix_room_joined = Some(is_joined);
        let button_text = if is_joined { "Go to the Robrix room" } else { "Join the Robrix room" };
        self.view.button(cx, ids!(join_robrix_room_button)).set_text(cx, button_text);
    }

    /// If the tips fit on one row, pad the shorter ones so that all of their hints
    /// are vertically aligned to start at the same height.
    /// Otherwise, show the dividers between the vertically-stacked tips.
    fn update_tips_layout(&self, cx: &mut Cx) {
        let grid_width = self.view.view(cx, ids!(tips_grid)).area().rect(cx).size.x;
        let uppers = [id!(workspace_tip), id!(settings_tip), id!(navigation_tip)]
            .map(|tip| self.view.view(cx, &[tip, id!(upper)]));
        let rects = uppers.each_ref().map(|upper| upper.area().rect(cx));
        let is_one_row = rects[0].size.x < grid_width / 2.0;
        let tallest = rects.iter().fold(0.0, |tallest, rect| rect.size.y.max(tallest));
        for (upper, rect) in uppers.iter().zip(rects) {
            let bottom_margin = if is_one_row { tallest - rect.size.y } else { 0.0 };
            if let Some(mut upper) = upper.borrow_mut()
                && upper.walk.margin.bottom != bottom_margin
            {
                upper.walk.margin.bottom = bottom_margin;
                upper.redraw(cx);
            }
        }
        for divider in [id!(tip_divider_1), id!(tip_divider_2)] {
            self.view.view(cx, &[divider]).set_visible(cx, !is_one_row);
        }
    }

    /// Lines up the Robrix room panel's details with its title, or centers them if the panel is too narrow.
    fn update_robrix_room_panel_layout(&mut self, cx: &mut Cx) {
        let header_width = self.view.view(cx, ids!(robrix_room_header)).area().rect(cx).size.x;
        let is_narrow = header_width < ROBRIX_ROOM_PANEL_CENTERING_WIDTH;
        let indent = self.view.view(cx, ids!(robrix_room_details_indent));
        if indent.visible() == !is_narrow {
            return;
        }
        indent.set_visible(cx, !is_narrow);
        let align_x = if is_narrow { 0.5 } else { 0.0 };
        if let Some(mut details) = self.view.view(cx, ids!(robrix_room_details)).borrow_mut() {
            details.layout.align.x = align_x;
        }
        if let Some(mut description) = self.view.label(cx, ids!(robrix_room_description)).borrow_mut() {
            description.align.x = align_x;
        }
        self.view.redraw(cx);
    }
}
