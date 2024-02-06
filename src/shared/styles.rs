use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_draw::shader::std::*;
    import makepad_widgets::theme_desktop_dark::*;
    

    // BEGIN LEGACY
    TITLE_TEXT = {
        font_size: (14),
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }

    REGULAR_TEXT = {
        font_size: (12),
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }

    TEXT_SUB = {
        font_size: (FONT_SIZE_SUB),
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }

    COLOR_PROFILE_CIRCLE = #xfff8ee
    COLOR_DIVIDER = #x00000018
    COLOR_DIVIDER_DARK = #x00000044

    // END LEGACY

    ICO_CREATE = dep("crate://self/resources/icons/create.svg")
    ICO_HOME = dep("crate://self/resources/icons/home.svg")
    ICO_DM = dep("crate://self/resources/icons/dm.svg")

    SPACE_FACTOR = 1.0 // Decrease for a more compact layout
    SPACE_0 = 0.0
    SPACE = 5.0
    SPACE_1 = (SPACE * 2 * (SPACE_FACTOR))
    SPACE_2 = (SPACE * 4 * (SPACE_FACTOR))

    MSPACE_0 = {top: (SPACE_0), right: (SPACE_0), bottom: (SPACE_0), left: (SPACE_0)}
    MSPACE_1 = {top: (SPACE), right: (SPACE), bottom: (SPACE), left: (SPACE)}
    MSPACE_H_1 = {top: (SPACE_0), right: (SPACE), bottom: (SPACE_0), left: (SPACE)}
    MSPACE_V_1 = {top: (SPACE), right: (SPACE_0), bottom: (SPACE), left: (SPACE_0)}
    MSPACE_2 = {top: (SPACE_1), right: (SPACE_1), bottom: (SPACE_1), left: (SPACE_1)}
    MSPACE_H_2 = {top: (SPACE_0), right: (SPACE_1), bottom: (SPACE_0), left: (SPACE_1)}
    MSPACE_V_2 = {top: (SPACE_1), right: (SPACE_0), bottom: (SPACE_1), left: (SPACE_0)}

    COLOR_BG = #xF0F0F0

    COLOR_U = #xFFFFFFFF
    COLOR_U_0 = #xFFFFFF00
    COLOR_U_2 = #xFFFFFFFF

    COLOR_D = #x000000FF
    COLOR_D_0 = #x00000000
    COLOR_D_1 = #x00000011
    COLOR_D_2 = #x00000028
    COLOR_D_3 = #x00000033
    COLOR_D_4 = #x00000044
    COLOR_D_5 = #x00000066
    COLOR_D_6 = #x00000088
    COLOR_D_7 = #x000000AA

    COLOR_ACCENT = #f00
    COLOR_SELECT = (COLOR_D_3)
    COLOR_HL = (COLOR_D_7)
    COLOR_TEXT = (COLOR_D_6)
    COLOR_META = (COLOR_D_4)

    FONT_SIZE_BASE = 7.0
    FONT_SIZE_CONTRAST = 2.75 // Greater values = greater font-size steps between font-formats (i.e. from H3 to H2)

    FONT_SIZE_1 = (FONT_SIZE_BASE + 5 * FONT_SIZE_CONTRAST)
    FONT_SIZE_2 = (FONT_SIZE_BASE + 4 * FONT_SIZE_CONTRAST)
    FONT_SIZE_3 = (FONT_SIZE_BASE + 3 * FONT_SIZE_CONTRAST)
    FONT_SIZE_4 = (FONT_SIZE_BASE + 2 * FONT_SIZE_CONTRAST)
    FONT_SIZE_P = (FONT_SIZE_BASE + 1 * FONT_SIZE_CONTRAST)

    Font = <Label> {
        draw_text: {
            text_style: {
                font: {path: dep("crate://self/resources/fonts/Inter-Regular.ttf")} },
        }
        text: "Font"
    }

    FontRegular = <Label> {
        width: Fill, height: Fit,
        draw_text: {text_style: {font: {path: dep("crate://self/resources/fonts/Inter-Regular.ttf")}}},
        text: "Regular Font"
    }
    FontRegularItalic = <Label> {
        width: Fill, height: Fit,
        draw_text: {text_style: {font: {path: dep("crate://self/resources/fonts/Inter-Italic.ttf")}}},
        text: "Regular Font"
    }
    FontBold = <Label> {
        width: Fill, height: Fit,
        draw_text: {text_style: {font: {path: dep("crate://self/resources/fonts/Inter-Bold.ttf")}}},
        text: "Bold Font"
    }
    FontBoldItalic = <Label> {
        width: Fill, height: Fit,
        draw_text: {text_style: {font: {path: dep("crate://self/resources/fonts/Inter-BoldItalic.ttf")}}},
        text: "Bold Font"
    }


    H1 = <FontBold> {
        draw_text: {
            text_style: { font_size: (FONT_SIZE_1) },
            color: (COLOR_HL)
        }
        text: "Headline H1"
    }
    H1italic = <FontBoldItalic> {
        draw_text: {
            text_style: { font_size: (FONT_SIZE_1) },
            color: (COLOR_HL)
        }
        text: "Headline H1"
    }
    H2 = <FontBold> {
        draw_text: {
            text_style: { font_size: (FONT_SIZE_2) },
            color: (COLOR_HL)
        }
        text: "Headline H2"
    }
    H2italic = <FontBoldItalic> {
        draw_text: {
            text_style: { font_size: (FONT_SIZE_2) },
            color: (COLOR_HL)
        }
        text: "Headline H2"
    }
    H3 = <FontBold> {
        draw_text: {
            text_style: { font_size: (FONT_SIZE_3) },
            color: (COLOR_HL)
        }
        text: "Headline H3"
    }
    H3italic = <FontBoldItalic> {
        draw_text: {
            text_style: { font_size: (FONT_SIZE_3) },
            color: (COLOR_HL)
        }
        text: "Headline H3"
    }
    H4 = <FontBold> {
        draw_text: {
            text_style: { font_size: (FONT_SIZE_4) },
            color: (COLOR_HL)
        }
        text: "Headline H4"
    }
    H4italic = <FontBoldItalic> {
        draw_text: {
            text_style: { font_size: (FONT_SIZE_4) },
            color: (COLOR_HL)
        }
        text: "Headline H4"
    }
    P = <FontRegular> {
        draw_text: {
            text_style: { font_size: (FONT_SIZE_P) },
            color: (COLOR_TEXT)
        }
        text: "Paragraph"
    }
    Meta = <P> {
        draw_text: {
            color: (COLOR_META)
        }
        text: "Meta data"
    }
    Pbold = <FontBold> {
        draw_text: {
            text_style: { font_size: (FONT_SIZE_P) },
            color: (COLOR_TEXT)
        }
        text: "Paragraph"
    }
    Pitalic = <FontRegularItalic> {
        draw_text: {
            text_style: { font_size: (FONT_SIZE_P) },
            color: (COLOR_TEXT)
        }
        text: "Paragraph"
    }
    Pbolditalic = <FontBoldItalic> {
        draw_text: {
            text_style: { font_size: (FONT_SIZE_P) },
            color: (COLOR_TEXT)
        }
        text: "Paragraph"
    }
    
    // COMPONENTS        
    LayoutBlock = <View> {
        width: Fill, height: Fill,
        spacing: (SPACE_0), flow: Right,
        margin: <MSPACE_0> {},
        padding: <MSPACE_0> {},
    }
    Rows = <LayoutBlock> { flow: Down }
    Columns = <LayoutBlock> { flow: Right }
    Filler = <View> { width: Fill, height: Fill, draw_bg: {color: (COLOR_U_0)}}

    OsHeader = <Rows> { height: 25.0 }
    OsFooter = <Rows> { height: 25.0 }

    Divider = <RectView> {
        margin: <MSPACE_0> {}, padding: <MSPACE_0> {},
        show_bg: true,
        draw_bg: {
            color: (COLOR_D_2),
            border_color: #0000,
            inset: vec4(0.0, 0.0, 0.0, 0.0)
        }
    }
    DividerH = <Divider> { height: 2.0, width: Fill, }
    DividerV = <Divider> { height: Fill, width: 2.0, }

    IconButton = <Button> {
        draw_icon: {
            svg_file: (ICO_CREATE),
            fn get_color(self) -> vec4 { return (COLOR_D_5) }
        }
        icon_walk: {width: 15.0, height: Fit}
        draw_bg: {
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                return sdf.result
            }
        }
        padding: <MSPACE_0> {},
        text: ""
    }    

    ImgButton = <Button> {
        draw_icon: {
            svg_file: (ICO_CREATE),
            fn get_color(self) -> vec4 { return (COLOR_D_5) }
        }
        icon_walk: {width: 15.0, height: Fit}
        draw_bg: {
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                return sdf.result
            }
        }
        padding: <MSPACE_0> {},
        text: ""
    }       

}
