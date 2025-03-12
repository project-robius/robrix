use makepad_widgets::Cx;

use gen_components::*;
use makepad_widgets::*;
use super::setting_page::SwitchPageAction;

live_design! {
    use link::widgets::*;
    use link::theme::*;
    use link::shaders::*;
    
    use link::gen_components::*;
    use crate::styles::*;

    use crate::setting::account_page::AccountPage;
    use crate::setting::notification_page::NotificationPage;
    use crate::setting::keyboard_page::KeyboardPage;

    pub RouterPage = {{RouterPage}}{
        height: Fill,
        width: Fill,
        flow: Down,
        background_visible: false,
        border_radius: 10.0,
        spacing: 12.0,
        padding: 12.0,
        scroll_bars: <GScrollBars>{},
        clip_x: true,
        clip_y: true,
        // background_color: #fff

        <GView> {
            height: Fill,
            width: Fill,
            flow: Down,
            border_radius: 10.0,
            // background_color: #fff
            padding: {left: 15, top: 15, right: 15, bottom: 15}

            app_router = <GRouter> {
                bar_pages = {
                    account_page = <GView> {
                        border_radius: 10.0,
                        // background_color: #fff
                        visible:true
                        <AccountPage> {}
                    } ,
                    notification_page = <GView> {
                        border_radius: 10.0,
                        visible:false
                        <NotificationPage> {}
                    }, 
                    keyboard_page = <GView> {
                        visible:false
                        border_radius: 10.0,
                        <KeyboardPage> {}
                    },
                }
            }
        }
    }
}

#[derive(Live, Widget, LiveHook)]
pub struct RouterPage {
    #[deref]
    pub deref_widget: GView,
}

impl Widget for RouterPage {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let _ = self.deref_widget.draw_walk(cx, scope, walk);

        let router = self.grouter(id!(app_router));
        router.borrow_mut().map(|mut router| {
            let _ = router
                .init(
                    ids!(account_page, notification_page, keyboard_page),
                    None,
                    None
                )
                // .active(id!(page1))
                .build(cx);
        });
        
        DrawStep::done()
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.deref_widget.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope); 
    }
}

impl WidgetMatchEvent for RouterPage {
    fn handle_actions(&mut self, cx: &mut Cx, actions :&Actions, _scope: &mut Scope) {
        let router = self.grouter(id!(app_router));
        for action in actions.iter() {
            match action.cast() {
                SwitchPageAction::AccountPage => {
                    router.nav_to(cx, id!(account_page));
                },
                SwitchPageAction::NotificationPage => {
                    router.nav_to(cx, id!(notification_page));
                }, 
                SwitchPageAction::KeyboardPage => {
                    router.nav_to(cx, id!(keyboard_page));
                },
                _ => {}
            }
        }

        router.handle_nav_events(cx, &actions);
    }
}

