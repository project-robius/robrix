//! A `LinkPreviewCard` view can display the base info of a web which link points to.

use makepad_widgets::*;

use crate::link_preview_cache::LinkPreview as LinkPreviewData;
use crate::shared::html_or_plaintext::HtmlOrPlaintextWidgetExt;
use crate::utils;


live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    
    use crate::shared::styles::*;
    use crate::shared::html_or_plaintext::*;

    pub LinkPreviewCard = {{LinkPreviewCard}} {
        width: Fill,
        height: Fit,
        padding: 10.0,
        spacing: 10.0,
        margin: 5,
        show_bg: true,
        draw_bg: {
            color: #000,
            instance border_radius: 4.0,
            fn pixel(self) -> vec4 {
                let border_color = #d4;
                let border_width = 1;
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                let body = #fff
                sdf.box(
                    1.,
                    1.,
                    self.rect_size.x - 2.0,
                    self.rect_size.y - 2.0,
                    self.border_radius
                )
                sdf.fill_keep(body)

                sdf.stroke(
                    border_color,
                    border_width
                )
                return sdf.result
            }
        },

        image = <View> {
            width: Fixed(120),
            height: Fixed(100),
            height: Fit,
            align: { x: 0.5, y: 0.5 },
            visible: true,
            image = <Image> {},
        },
        <View> {
            flow: Down,
            width: Fill,
            height: Fit,
            spacing: 5,
            title = <View> {
                width: Fill,
                height: Fit,
                padding: { bottom: 5.0 },
                text = <HtmlOrPlaintext> {}
            }

            description = <View> {
                width: Fill,
                height: Fit,    // Label height must be set to Fit, and it's parent, brothers.
                visible: true,
                text = <Label> {
                    width: Fill,
                    height: Fit,
                    draw_text: {
                        wrap: Word,
                        color: (MESSAGE_TEXT_COLOR),
                        text_style: <MESSAGE_TEXT_STYLE>{},
                    }
                }
            
            }
        }
    }
}


#[derive(LiveHook, Live, Widget)]
pub struct LinkPreviewCard {
    #[deref] view: View,
}

impl Widget for LinkPreviewCard {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope)
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl LinkPreviewCard {

    /// Sets the link preview card content, making the preview card visible and the plaintext invisible.
    pub fn show_card(
        &mut self, cx: &mut Cx, card: &LinkPreviewData
    )
    {
        let title = format!("<a href='{}'>{}</a>", card.url, card.title.as_ref().unwrap());
        self.html_or_plaintext(id!(title.text)).show_html(cx, title);

        if let Some(d) = card.description.as_ref(){
            self.label(id!(description.text)).set_text(cx, d);
            self.view(id!(description)).set_visible(cx, true);
        } else {
            self.view(id!(description)).set_visible(cx, false);
        };

        if let Some(image_data) = &card.image {
            let image_ref = self.image(id!(image.image));
            let _ = utils::load_png_or_jpg(&image_ref, cx, image_data)
                    .map(|()| image_ref.size_in_pixels(cx).unwrap_or_default());
            self.view(id!(image)).set_visible(cx, true);
        } else {
            self.view(id!(image)).set_visible(cx, false);
        };
    }
}

impl LinkPreviewCardRef {

    /// See [`LinkPreviewCard::show_card()`].
    pub fn show_card(&self, cx: &mut Cx, card: &LinkPreviewData) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_card(cx, card);
        }
    }
}

