use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;
    import crate::shared::helpers::*;
    import crate::shared::adaptive_layout_view::AdaptiveLayoutView;

    import crate::home::rooms_list::RoomsList;

    ICON_COLLAPSE = dep("crate://self/resources/icons/collapse.svg")
    ICON_ADD = dep("crate://self/resources/icons/add.svg")

    CollapsableTitle = <View> {
        width: Fill, height: Fit
        flow: Right, spacing: 10.
        align: {x: 0.0, y: 0.5}
        collapse_icon = <Icon> {
            draw_icon: {
                svg_file: (ICON_COLLAPSE),
                uniform rotation_angle: -90.0,
                fn get_color(self) -> vec4 {
                    return #666;
                }

                // Support rotation of the icon
                fn clip_and_transform_vertex(self, rect_pos: vec2, rect_size: vec2) -> vec4 {
                    let clipped: vec2 = clamp(
                        self.geom_pos * rect_size + rect_pos,
                        self.draw_clip.xy,
                        self.draw_clip.zw
                    )
                    self.pos = (clipped - rect_pos) / rect_size

                    // Calculate the texture coordinates based on the rotation angle
                    let angle_rad = self.rotation_angle * 3.14159265359 / 180.0;
                    let cos_angle = cos(angle_rad);
                    let sin_angle = sin(angle_rad);
                    let rot_matrix = mat2(
                        cos_angle, -sin_angle,
                        sin_angle, cos_angle
                    );
                    self.tex_coord1 = mix(
                        self.icon_t1.xy,
                        self.icon_t2.xy,
                        (rot_matrix * (self.pos.xy - vec2(0.5))) + vec2(0.5)
                    );

                    return self.camera_projection * (self.camera_view * (self.view_transform * vec4(
                        clipped.x,
                        clipped.y,
                        self.draw_depth + self.draw_zbias,
                        1.
                    )))
                }
            }
            icon_walk: {width: 12, height: 12}
        }

        title = <Label> {
            draw_text: {
                color: #x0,
                text_style: <TITLE_TEXT>{ font_size: 11}
            }
        }

        <View> {
            width: Fill
        }

        add_icon = <View> {
            width: Fit
            visible: false
            padding: {right: 10}
            align: {x: 0.5, y: 0.5}
            <Icon> {
                icon_walk: {width: 10, height: 10}
                draw_icon: {
                    svg_file: (ICON_ADD),
                    fn get_color(self) -> vec4 {
                        return #1C274C;
                    }
                }
            }
        }
    }

    RoomsSideBar = <AdaptiveLayoutView> {
        composition: {
            desktop: {
                padding: {top: 20., left: 20., right: 20.}
                flow: Down, spacing: 10
                width: 400, height: Fill
                visibility: Visible
            },
            mobile: {
                padding: {top: 17., left: 17., right: 17.}
                flow: Down, spacing: 7
                width: Fill, height: Fill
                visibility: Visible
            }
        }
        show_bg: true,
        draw_bg: {
            instance bg_color: (COLOR_PRIMARY)
            instance border_color: #f2f2f2
            instance border_width: 0.003

            // Draws a right-side border
            fn pixel(self) -> vec4 {
                if self.pos.x > 1.0 - self.border_width {
                    return self.border_color;
                } else {
                    return self.bg_color;
                }
            }
        }
        <Label> {
            text: "Home"
            draw_text: {
                color: #x0
                text_style: <TITLE_TEXT>{}
            }
        }
        <View> {
            flow: Down, spacing: 20
            padding: {top: 20}
            width: Fill, height: Fit
            <CollapsableTitle> {
                title = {
                    text: "People"
                }
            }
            <CollapsableTitle> {
                title = {
                    text: "Channels"
                }
            }
            <CollapsableTitle> {
                title = {
                    text: "Rooms"
                }
                collapse_icon = {
                    draw_icon: { rotation_angle: 0. }
                }
                add_icon = {
                    visible: true
                }
            }
        }
        <RoomsList> {}
    }
}
