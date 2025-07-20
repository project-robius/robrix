use makepad_widgets::*;

use crate::sliding_sync::{submit_async_request, MatrixRequest};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    IMG_APP_LOGO = dep("crate://self/resources/robrix_logo_alpha.png")

    pub LinkPreviewCard = {{LinkPreviewCard}}<RoundedView> {
        width: Fill, height: Fit,
        show_bg: true,
        draw_bg: {
          color: #EEF2F4,
          border_radius: 3.0
        }

        image = <Image> {
          visible: false,
          fit: Smallest,
          source: (IMG_APP_LOGO),
        }

        link_preview_info_view = <View> {
            visible: false,
            width: Fill, height: Fit,
            flow: Down,
            title = <Label> {
                width: Fill, height: Fit,
                flow: RightWrap,
                draw_text: {
                    wrap: Word,
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <THEME_FONT_BOLD> { font_size: 12 },
                }
                text: "",
            }
            description = <Label> {
                width: Fill, height: Fit,
                flow: RightWrap,
                draw_text: {
                    wrap: Word,
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <MESSAGE_TEXT_STYLE> { font_size: (MESSAGE_FONT_SIZE) },
                }
                text: "",
            }
        }
    }
}

#[derive(Clone, Debug, DefaultNone)]
pub enum LinkPreviewCardState {
    Requested,
    Loaded {
        url: String,
        preview: Option<url_preview::Preview>,
    },
    None,
}

#[derive(Live, LiveHook, Widget)]
pub struct LinkPreviewCard {
  #[deref]
  view: View,

  #[rust]
  url: String,
  #[rust]
  state: LinkPreviewCardState,
}

impl Widget for LinkPreviewCard {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
      if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(loaded @ LinkPreviewCardState::Loaded { url, .. }) = action.downcast_ref() {
                    if self.url == *url {
                        self.state = loaded.clone();
                        self.redraw(cx);
                    }
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl LinkPreviewCard {
  pub fn populate_link_preview_card(&mut self, cx: &mut Cx, url: String) {
    self.url = url.clone();

    match &self.state {
      LinkPreviewCardState::Loaded { url, preview } => {
        if self.url == *url {
          if let Some(preview) = preview {
            let link_preview_info = self.view(id!(link_preview_info_view));
            let image = self.image(id!(image));
            image.set_visible(cx, true);

            link_preview_info.set_visible(cx, true);
            link_preview_info.label(id!(title)).set_text(cx, preview.title.as_deref().unwrap_or(""));
            link_preview_info.label(id!(description)).set_text(cx, preview.description.as_deref().unwrap_or(""));
          }
        }
        return;
      }
      LinkPreviewCardState::None => {
        submit_async_request(MatrixRequest::GetLinkPreviewDetails { url: url.clone() });
        self.state = LinkPreviewCardState::Requested;
      }
      _ => {}
    }
  }
}

impl LinkPreviewCardRef{
  pub fn set_url(&mut self, url: &str) {
    if let Some(mut inner) = self.borrow_mut() {
      inner.url = url.to_string();
    }
  }
}