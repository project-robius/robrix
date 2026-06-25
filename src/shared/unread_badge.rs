//! This module defines a badge that shows the count of unread mentions (in red)
//! or unread messages (in gray).

use makepad_widgets::*;


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.UnreadBadge = #(UnreadBadge::register_widget(vm)) {

        width: 27, height: 18,
        align: Align{ x: 0.5, y: 0.5 }
        flow: Overlay,
        // Let the badge's fade-out/glow effect render beyond the badge's rect.
        clip_x: false,
        clip_y: false,

        rounded_view := View {
            width: Fill,
            height: Fill,
            show_bg: true,
            clip_x: false,
            clip_y: false,

            draw_bg +: {
                badge_color: instance((COLOR_UNREAD_BADGE_MESSAGES)),
                border_radius: instance(4.0)
                // A larger border size results in a smaller oval
                border_size: instance(2.0)
                // For unread mention badges only, we fade through a lighter color to reduce aliasing effects
                // on lower-res screens, since red on purple looks blocky/pixellated otherwise.
                fade_color: instance(#xFFC8B0)
                fade_radius: uniform(5.0)
                // Controls the transition of the outer border. 
                // 0.0 is a crisp solid badge, 1.0 is a soft fading/dissolve transition.
                soft: instance(0.0)

                vertex: fn() {
                    let m = self.fade_radius
                    return self.clip_and_transform_vertex(
                        self.rect_pos - vec2(m),
                        self.rect_size + vec2(m * 2.0)
                    )
                }

                pixel: fn() {
                    let m = self.fade_radius
                    let rs3 = self.rect_size + vec2(m * 2.0)
                    let sdf = Sdf2d.viewport(self.pos * rs3)
                    let bw = self.rect_size.x - (self.border_size * 2.0)
                    let bh = self.rect_size.y - 2.0
                    let bx = m + self.border_size
                    let by = m + 1.0
                    let rad = max(1.0, self.border_radius)
                    sdf.box(bx, by, bw, bh, rad)
                    let dist = sdf.shape
                    let half = bh * 0.5
                    let aa = clamp(0.5 - dist, 0.0, 1.0)
                    let band_start = -half * 0.45
                    let t = clamp((dist - band_start) / (m - band_start), 0.0, 1.0)
                    let s = t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
                    let warm = mix(self.badge_color.rgb, self.fade_color.rgb, s)
                    let dissolve = 1.0 - s
                    let color = mix(self.badge_color.rgb, warm, self.soft)
                    let alpha = mix(aa, dissolve, self.soft)
                    sdf.clear(vec4(color, alpha))
                    return sdf.result;
                }
            }
        }
        // Label that displays the unread message count
        label_count := Label {
            padding: 0,
            width: Fit,
            height: Fit,
            flow: Right, // do not wrap
            text: "",
            draw_text +: {
                color: #ffffff,
                text_style: theme.font_regular {font_size: 8.0},
            }
        }
    }
}


#[derive(Script, ScriptHook, Widget)]
pub struct UnreadBadge {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[live] is_marked_unread: bool,
    #[live] unread_mentions: u64,
    #[live] unread_messages: u64,
}

impl Widget for UnreadBadge {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {

        /// Helper function to format the badge's rounded rectangle.
        ///
        /// The rounded rectangle needs to be wider for longer text.
        /// It also adds a plus sign at the end if the unread count is greater than 99. 
        fn format_border_and_truncation(count: u64) -> (f64, &'static str) {
            let (border_size, plus_sign) = if count > 99 {
                (0.0, "+")
            } else if count > 9 {
                (2.0, "")
            } else {
                (5.0, "")
            };
            (border_size, plus_sign)
        }

        // If there are unread mentions, show red badge and the number of unread mentions
        if self.unread_mentions > 0 {
            let (border_size, plus_sign) = format_border_and_truncation(self.unread_mentions);
            self.label(cx, ids!(label_count))
                .set_text(cx, &format!("{}{plus_sign}", std::cmp::min(self.unread_mentions, 99)));
            let mut rounded_view = self.view(cx, ids!(rounded_view));
            script_apply_eval!(cx, rounded_view, {
                draw_bg +: {
                    border_size: #(border_size),
                    // Solid red core fading out through a lighter warm color.
                    badge_color: #xFF1133,
                    fade_color: #xFFC8B0,
                    soft: 1.0
                }
            });
            self.visible = true;
        }
        // If there are no unread mentions but this is marked as unread, show the badge as a dot.
        else if self.is_marked_unread {
            self.label(cx, ids!(label_count)).set_text(cx, "");
            let mut rounded_view = self.view(cx, ids!(rounded_view));
            script_apply_eval!(cx, rounded_view, {
                draw_bg +: {
                    border_size: 6.0, // larger value = smaller badge size
                    badge_color: mod.widgets.COLOR_UNREAD_BADGE_MARKED,
                    soft: 0.0
                }
            });
            self.visible = true;
        }
        // If there are no unread mentions but there are unread messages, show gray badge and the number of unread messages
        else if self.unread_messages > 0 {
            let (border_size, plus_sign) = format_border_and_truncation(self.unread_messages);
            self.label(cx, ids!(label_count))
                .set_text(cx, &format!("{}{plus_sign}", std::cmp::min(self.unread_messages, 99)));
            let mut rounded_view = self.view(cx, ids!(rounded_view));
            script_apply_eval!(cx, rounded_view, {
                draw_bg +: {
                    border_size: #(border_size),
                    badge_color: mod.widgets.COLOR_UNREAD_BADGE_MESSAGES,
                    soft: 0.0
                }
            });
            self.visible = true;
        }
        else {
            // If there are no unreads of any kind, hide the badge
            self.visible = false;
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl UnreadBadgeRef {
    /// Sets the unread mentions and messages counts without explicitly redrawing the badge.
    pub fn update_counts(&self, is_marked_unread: bool, num_unread_mentions: u64, num_unread_messages: u64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.is_marked_unread = is_marked_unread;
            inner.unread_mentions = num_unread_mentions;
            inner.unread_messages = num_unread_messages;
            inner.visible = is_marked_unread || num_unread_mentions > 0 || num_unread_messages > 0;
        }
    }
}
