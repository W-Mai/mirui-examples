//! ESP32-C3 lite demo: small widget tree (no LazyList / no SimTimeline)
//! that fits in the SoC's 400 KB SRAM heap budget. Demonstrates Theme
//! cycle, Slider, Switch, ProgressBar wiring without the heavy alloc
//! pattern that crashes the desktop / qemu `widgets::build` demo on
//! ESP32-C3 NuttX (every theme cycle does a subtree dirty mark + Vec
//! grow, which fragments the 363 KB heap to OOM around the 15th
//! cycle).

use mirui::app::{App, RendererFactory};
use mirui::ui::widgets::{ProgressBar, Slider, Switch};
use mirui::ecs::Entity;
use mirui::app::plugins::StdInstantClockPlugin;
use mirui::prelude::*;
use mirui::surface::Surface;
use mirui::types::Fixed;
use mirui::ui::theme::{self, ColorToken};
use mirui::ui::Theme;
use mirui_macros::ui;

const ACCENT: ColorToken = ColorToken::custom("accent");

struct ThemeCycleIndex(u8);

fn dark_with_accent() -> Theme {
    Theme::dark().with(ACCENT, Color::rgb(255, 200, 60))
}
fn light_with_accent() -> Theme {
    Theme::light().with(ACCENT, Color::rgb(220, 60, 90))
}
fn custom_theme() -> Theme {
    Theme::dark().with_many([
        (ColorToken::Primary, Color::rgb(255, 105, 180)),
        (ColorToken::OnPrimary, Color::rgb(20, 20, 30)),
        (ColorToken::Surface, Color::rgb(38, 28, 50)),
        (ColorToken::SurfaceVariant, Color::rgb(70, 50, 90)),
        (ColorToken::OnSurface, Color::rgb(245, 235, 255)),
    ])
}

mirui_macros::timer!(Cycle, every: 3_000, |world, entity| {
    let next = world
        .get::<ThemeCycleIndex>(entity)
        .map(|i| (i.0 + 1) % 3)
        .unwrap_or(0);
    world.insert(entity, ThemeCycleIndex(next));
    let theme = match next {
        0 => dark_with_accent(),
        1 => light_with_accent(),
        _ => custom_theme(),
    };
    theme::set_theme(world, theme);
});

pub fn build<S, F>(app: &mut App<S, F>) -> Entity
where
    S: Surface,
    F: RendererFactory<S>,
{
    app.add_plugin(StdInstantClockPlugin);
    // FpsSummaryPlugin spams 2 lines/sec on USB-CDC; the host-side
    // serial buffer fills up faster than the host drains it, blocking
    // mirui's eprintln paths and looking like a hang. Re-enable for
    // perf debugging only when serial is being actively read.
    // app.add_plugin(FpsSummaryPlugin::default());
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
        column (direction: FlexDirection::Column, grow: 1.0) {
            title (
                text: "mirui",
                text_color: ColorToken::OnSurface,
                bg_color: ACCENT,
                border_radius: 6,
                padding: Padding::all(6),
                height: 24
            ) {}
            spacer1 (height: 6) {}
            slider_track (
                bg_color: ColorToken::SurfaceVariant,
                height: 14,
                border_radius: 7
            ) [
                {
                    let mut s = Slider::new(Fixed::ZERO, Fixed::from_int(100));
                    s.value = Fixed::from_int(40);
                    s
                },
            ] {}
            spacer2 (height: 6) {}
            progress_track (
                bg_color: ColorToken::SurfaceVariant,
                height: 8,
                border_radius: 4
            ) [
                ProgressBar { value: 0.4, ..ProgressBar::default() },
            ] {}
            spacer3 (height: 6) {}
            switch_track (
                bg_color: ColorToken::SurfaceVariant,
                width: 36,
                height: 18,
                border_radius: 9
            ) [
                Switch::new(),
            ] {}
        }
    };
    root
}
