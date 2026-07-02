//! Heap-constrained variant of the widgets demo. gallery's
//! `widgets::build_widgets` builds a LazyList + SimTimeline that
//! fragments the 363 KB ESP32-C3 heap to OOM around the 15th theme
//! cycle; this file keeps only Slider / Switch / ProgressBar and
//! reuses gallery's theme cycle machinery.

use mirui::app::plugins::StdInstantClockPlugin;
use mirui::gallery::demos::widgets::{ACCENT, Cycle, ThemeCycleIndex, dark_with_accent};
use mirui::prelude::*;
use mirui::ui::widgets::{ProgressBar, Slider, Switch, Text};

pub fn build<S, F>(app: &mut App<S, F>) -> Entity
where
    S: Surface,
    F: RendererFactory<S>,
{
    app.add_plugin(StdInstantClockPlugin);
    app.world.insert_resource(dark_with_accent());
    let cycle = Cycle::install(&mut app.world);
    app.world.insert(cycle, ThemeCycleIndex(0));

    let world = &mut app.world;
    let root = WidgetBuilder::new(world)
        .bg_color(ColorToken::Surface)
        .layout(LayoutStyle {
            direction: FlexDirection::Column,
            padding: Padding::all(8),
            ..Default::default()
        })
        .id();
    ui! {
        :(
            parent: root
            world: world
        :)
        Column (grow: 1.0) {
            View (
                bg_color: ACCENT,
                text_color: ColorToken::OnSurface,
                border_radius: 6,
                padding: Padding::all(6),
                height: 24
            ) {
                Text ("mirui")
            }
            View (height: 6)
            View (bg_color: ColorToken::SurfaceVariant, height: 14, border_radius: 7) [
                {
                    let mut s = Slider::new(Fixed::ZERO, Fixed::from_int(100));
                    s.value = Fixed::from_int(40);
                    s
                },
            ]
            View (height: 6)
            View (bg_color: ColorToken::SurfaceVariant, height: 8, border_radius: 4) [
                ProgressBar { value: 0.4, ..ProgressBar::default() },
            ]
            View (height: 6)
            View (
                bg_color: ColorToken::SurfaceVariant,
                width: 36,
                height: 18,
                border_radius: 9
            ) [
                Switch::new(),
            ]
        }
    };
    root
}
