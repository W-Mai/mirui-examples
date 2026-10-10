//! 4-band pure-color probe: red / green / blue / white from top to bottom.
//! Use to disambiguate panel byte order, BGR/RGB layout, and rotation:
//! swap `widgets::build` for `color_probe::build` in `lib.rs::mirui_main`.

use mirui::prelude::*;

pub fn build<S, F>(app: &mut App<S, F>) -> Entity
where
    S: Surface,
    F: RendererFactory<S>,
{
    let root = app.spawn_root().id();
    app.compose(root, color_bands());
    root
}

#[compose]
fn color_bands() {
    ui! {
        Column (grow: 1.0) {
            View (bg_color: Color::rgb(255, 0, 0), grow: 1.0)
            View (bg_color: Color::rgb(0, 255, 0), grow: 1.0)
            View (bg_color: Color::rgb(0, 0, 255), grow: 1.0)
            View (bg_color: Color::rgb(255, 255, 255), grow: 1.0)
        }
    };
}
