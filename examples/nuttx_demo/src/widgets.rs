//! NuttX runner for gallery's widgets showcase. Wraps
//! `mirui::gallery::demos::widgets::setup_app` after spawning a
//! viewport-sized root against `/dev/fb0`.

use mirui::prelude::*;

pub fn build<S, F>(app: &mut App<S, F>) -> Entity
where
    S: Surface,
    F: RendererFactory<S>,
{
    let info = app.backend.display_info();
    let root = WidgetBuilder::new(&mut app.world)
        .bg_color(ColorToken::Surface)
        .layout(LayoutStyle {
            direction: FlexDirection::Column,
            width: Dimension::px(info.width as i32),
            height: Dimension::px(info.height as i32),
            ..Default::default()
        })
        .id();

    mirui::gallery::demos::widgets::setup_app(app, root);
    root
}
