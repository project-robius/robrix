use makepad_widgets::Cx;

pub mod tsp_settings;

// pub mod create_wallet_modal;


pub fn live_design(cx: &mut Cx) {
    // create_wallet_modal::live_design(cx);
    tsp_settings::live_design(cx);
}
