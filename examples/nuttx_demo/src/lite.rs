//! Compact Slider, Switch, and ProgressBar demo with a three-theme cycle.

use mirui::app::plugins::StdInstantClockPlugin;
use mirui::gallery::demos::widgets::{ACCENT, custom_theme, dark_with_accent, light_with_accent};
use mirui::prelude::*;
use mirui::ui::widgets::{ProgressBar, Slider, Switch, Text};

#[derive(Default)]
#[model]
struct ThemeCycle {
    index: u8,
}

#[model]
impl ThemeCycle {
    fn advance(&mut self) {
        self.index = (self.index + 1) % 3;
    }

    fn current_theme(&self) -> Theme {
        match self.index {
            0 => dark_with_accent(),
            1 => light_with_accent(),
            _ => custom_theme(),
        }
    }
}

#[timer(every = 3_000)]
fn cycle(theme_cycle: model!(ThemeCycle), commands: commands!()) {
    theme_cycle.advance();
    commands.set_theme(theme_cycle.current_theme());
}

pub fn build<S, F>(app: &mut App<S, F>) -> Entity
where
    S: Surface,
    F: RendererFactory<S>,
{
    app.add_plugin(StdInstantClockPlugin);
    app.add_resource(dark_with_accent());
    let theme_cycle = app.add_model(ThemeCycle::default());
    app.add_timer(cycle::timer(theme_cycle));

    let root = app
        .spawn_root()
        .layout(LayoutStyle {
            direction: FlexDirection::Column,
            padding: Padding::all(8),
            ..Default::default()
        })
        .id();
    app.compose(root, controls());
    root
}

#[compose]
fn controls() {
    ui! {
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
                ProgressBar {
                    value: 0.4,
                    ..ProgressBar::default()
                },
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
}
