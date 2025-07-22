use std::sync::Arc;

use linkify::LinkFinder;
use makepad_widgets::*;

use crate::{sliding_sync::{submit_async_request, MatrixRequest}, utils};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    pub LinkPreviewCard = {{LinkPreviewCard}}<RoundedView> {
        width: Fill, height: Fit,
        show_bg: true,
        cursor: Hand,
        draw_bg: {
          color: #EEF2F4,
          border_radius: 3.0
        }

        link_thumbnail_preview_view = <View> {
            visible: false,
            width: Fit, height: Fit,
            image = <Image> {
              fit: Stretch
            }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkPreviewData {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub raw_image: Option<Arc<Vec<u8>>>,
}

#[derive(Clone, Debug, DefaultNone, PartialEq, Eq)]
pub enum LinkPreviewCardState {
    Requested,
    Loaded {
        url: String,
        preview: Option<LinkPreviewData>,
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
                  if self.url == *url && self.state != *loaded {
                      self.state = loaded.clone();
                      self.redraw(cx);
                  }
              }
          }
      }

      if let Hit::FingerUp(fue) = event.hits(cx, self.view.area()) {
        if fue.is_primary_hit() && fue.was_tap() {
          if let Err(e) = robius_open::Uri::new(&self.url).open() {
            error!("Failed to open URL {:?}. Error: {:?}", self.url, e);
          }
        }
      }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
      match &self.state {
          LinkPreviewCardState::Loaded { preview: Some(preview), .. } => {
              let link_preview_info = self.view(id!(link_preview_info_view));
              let link_thumbnail_preview_view = self.view(id!(link_thumbnail_preview_view));

              if let Some(raw_image) = &preview.raw_image {
                  let image = self.image(id!(link_thumbnail_preview_view.image));
                  let _ = utils::load_png_or_jpg(&image, cx, raw_image);
                  link_thumbnail_preview_view.set_visible(cx, true);
              }

              link_preview_info.set_visible(cx, true);
              link_preview_info.label(id!(title)).set_text(cx, preview.title.as_deref().unwrap_or(""));
              link_preview_info.label(id!(description)).set_text(cx, preview.description.as_deref().unwrap_or(""));
          }
          LinkPreviewCardState::None => {
              // If no state is set, we can request the preview
              self.state = LinkPreviewCardState::Requested;
              submit_async_request(MatrixRequest::GetLinkPreviewDetails{
                url: self.url.clone(),
              });
          }
          _ => {}
      }
      self.view.draw_walk(cx, scope, walk)
    }
}

impl LinkPreviewCard {
  pub fn set_card_info(&mut self, cx: &mut Cx, url: String) {
    if self.url != url {
      self.url = url.clone();
      self.state = LinkPreviewCardState::Requested;
      submit_async_request(MatrixRequest::GetLinkPreviewDetails {
        url: url.clone(),
      });
    }
    self.view.set_visible(cx, true);
  }
}

impl LinkPreviewCardRef{
  pub fn set_card_info(
    &mut self,
    cx: &mut Cx,
    link: &str
  ) {
    if let Some(mut inner) = self.borrow_mut() {
      inner.set_card_info(cx, link.to_owned());
    }
  }
}

pub fn populate_link_preview_card_content(
  cx: &mut Cx,
  link_preview_card_content_widget: &mut LinkPreviewCardRef,
  body: &str
) {
  let links = extract_links(body, &[linkify::LinkKind::Url]);
  if !links.is_empty() {
    // We just show the last link preview for now
    let link = links.last().unwrap();
    link_preview_card_content_widget
      .set_card_info(cx, link);
  }
}

const IGNORED_DOMAINS: &[&str] = &[
    "matrix.to",
    "matrix.io",
];

pub fn extract_links(
  text: &str,
  kinds: &[linkify::LinkKind],
) -> Vec<String> {
    let mut finder = LinkFinder::new();
    finder.kinds(kinds);
    let links = finder.links(text);

    let mut result = Vec::new();

    for link in links {
        let start = link.start();
        let link_text = link.as_str();

        if IGNORED_DOMAINS.iter().any(|domain| link_text.contains(domain)) {
            continue;
        }

        if text.get(..start).is_some_and(|before| {
            let lower = before.to_ascii_lowercase();
            lower.rfind("href=").is_some_and(|i| {
                lower[i..].starts_with("href=\"") || lower[i..].starts_with("href='")
            })
        }) {
            continue;
        }

        result.push(link_text.to_string());
    }
  result
}