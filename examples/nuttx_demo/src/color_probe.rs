//! 4-band pure-color probe: red / green / blue / white from top to bottom.
//! Use to disambiguate panel byte order, BGR/RGB layout, and rotation:
//! swap `widgets::build` for `color_probe::build` in `lib.rs::mirui_main`.

use mirui::app::{App, RendererFactory};
use mirui::ecs::Entity;
use mirui::prelude::*;
use mirui::surface::Surface;
use mirui_macros::ui;

pub fn build<S, F>(app: &mut App<S, F>) -> Entity
where
    S: Surface,
    F: RendererFactory<S>,
{
    let world = &mut app.world;
    let root = WidgetBuilder::new(world).id();
    ui! {
        :(
            parent: root
            world: world
        :)
        column (direction: FlexDirection::Column, grow: 1.0) {
            red    (bg_color: Color::rgb(255, 0,   0),   grow: 1.0) {}
            green  (bg_color: Color::rgb(0,   255, 0),   grow: 1.0) {}
            blue   (bg_color: Color::rgb(0,   0,   255), grow: 1.0) {}
            white  (bg_color: Color::rgb(255, 255, 255), grow: 1.0) {}
        }
    };
    root
}
