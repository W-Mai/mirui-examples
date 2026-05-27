//! ESP showcase exercising mirui v0.14 ThemedColor + v0.14.1
//! `theme::set_theme` + v0.14.2 `timer!` macro:
//! - tab "List" → LazyList of 50 rows; rows use `Surface` /
//!   `OnSurface` tokens.
//! - tab "Form" → Switch + Slider + ProgressBar with the Slider
//!   value pushed onto the ProgressBar by `slider_to_progress_system`.
//!   Every widget colour comes from its built-in default token.
//! - tab "Theme" → two colour blocks (one for `Primary`, one for a
//!   user-defined `accent` token). A `timer!`-generated `Cycle`
//!   rotates Dark / Light / Custom every 3 s; the whole UI repaints
//!   in the new palette next frame.

use mirui::anim::ease;
use mirui::components::{LazyList, LazyListBinder, LazyListPool};
use mirui::components::{ProgressBar, Slider, Switch, TabBar, TabContent, Text};
use mirui::event::scroll::{ScrollAxis, ScrollConfig, ScrollOffset};
use mirui::event::sim::{SimAction, SimTimeline, sim_timeline_system};
use mirui::prelude::*;
use mirui::types::{Color, DimPoint, Dimension, Fixed};
use mirui::widget::dirty::Dirty;
use mirui::widget::theme::{self, ColorToken};
use mirui::widget::{Children, OffscreenRender, Theme};

use alloc::format;
use alloc::vec::Vec;

const ROW_H: i32 = 12;
const POOL_SIZE: usize = 12;
const ITEM_COUNT: u32 = 50;

/// User-defined token. The "Theme" tab's lower colour block reads
/// it; each preset theme below binds it to a different colour.
const ACCENT: ColorToken = ColorToken::custom("accent");

/// `slider_to_progress_system` reads `FormSlider`, writes `FormProgress`.
struct FormSlider;
struct FormProgress;

/// Counter component on the cycle timer entity; the `timer!` callback
/// reads it to pick the next preset.
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
        (ColorToken::Success, Color::rgb(255, 200, 60)),
        (ColorToken::Surface, Color::rgb(38, 28, 50)),
        (ColorToken::SurfaceVariant, Color::rgb(70, 50, 90)),
        (ColorToken::OnSurface, Color::rgb(245, 235, 255)),
        (ColorToken::OnSurfaceVariant, Color::rgb(180, 150, 200)),
        (ACCENT, Color::rgb(140, 200, 220)),
    ])
}

fn row_binder(world: &mut World, entity: Entity, index: u32) {
    let label = format!("Row {index}");
    if let Some(t) = world.get_mut::<Text>(entity) {
        t.0 = label.into_bytes();
    } else {
        world.insert(entity, Text(label.into_bytes()));
    }
}

/// Push Slider.value → ProgressBar.value every frame. Demonstrates
/// runtime state coupling without going through gesture handlers.
#[mirui::system]
fn slider_to_progress_system(world: &mut World) {
    let sliders: Vec<Entity> = world.query::<FormSlider>().collect();
    let mut value = None;
    for e in sliders {
        if let Some(s) = world.get::<Slider>(e) {
            value = Some(s.value.to_f32() / 100.0);
        }
    }
    let Some(v) = value else { return };
    let bars: Vec<Entity> = world.query::<FormProgress>().collect();
    for e in bars {
        if let Some(pb) = world.get_mut::<ProgressBar>(e) {
            if (pb.value - v).abs() > 0.001 {
                pb.value = v;
                world.insert(e, Dirty);
            }
        }
    }
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

pub fn setup<B: mirui::surface::FramebufferAccess>(app: &mut App<B>) {
    app.add_plugin(mirui::plugins::InputFeedbackPlugin::default());
    // ESP heap is 200 KB total; 32 KiB is enough for a couple of
    // small-widget buffers (Switch is 40×20 = 1.6 KB) without
    // crowding out the rest of the demo. The pool's default is
    // disabled (budget=0) so OffscreenRender falls through to
    // inline unless the app opts in here.
    app.with_offscreen_pool_budget(32 * 1024);
    app.add_system(sim_timeline_system::system());
    app.add_system(slider_to_progress_system::system());

    app.world.insert_resource(dark_with_accent());
    let cycle_e = Cycle::install(&mut app.world);
    app.world.insert(cycle_e, ThemeCycleIndex(0));

    let root = WidgetBuilder::new(&mut app.world)
        .bg_color(ColorToken::Surface)
        .layout(LayoutStyle {
            direction: FlexDirection::Column,
            width: Dimension::px(128),
            height: Dimension::px(128),
            ..Default::default()
        })
        .id();

    let tabs = mirui_macros::ui! {
        :(
            parent: root
            world: &mut app.world
        :)

        tabs (
            bg_color: ColorToken::SurfaceVariant,
            width: 128,
            height: 14
        ) [
            TabBar::new(3).with_indicator_height(2),
        ] {
            tab0 (
                text: "List",
                text_color: ColorToken::OnSurface,
                grow: 1.0,
                align: AlignItems::Center,
                justify: JustifyContent::Center
            ) {}
            tab1 (
                text: "Form",
                text_color: ColorToken::OnSurface,
                grow: 1.0,
                align: AlignItems::Center,
                justify: JustifyContent::Center
            ) {}
            tab2 (
                text: "Thm",
                text_color: ColorToken::OnSurface,
                grow: 1.0,
                align: AlignItems::Center,
                justify: JustifyContent::Center
            ) {}
        }
    };

    // Tab A: LazyList of 50 rows.
    let list = mirui_macros::ui! {
        :(
            parent: root
            world: &mut app.world
        :)

        list (
            bg_color: ColorToken::Surface,
            width: 128,
            height: 114
        ) [
            TabContent {
                tab_bar: tabs,
                index: 0,
            },
            LazyList::new(ITEM_COUNT, ROW_H, POOL_SIZE as u8),
            LazyListBinder { bind: row_binder },
            ScrollOffset {
                x: Fixed::ZERO,
                y: Fixed::ZERO,
            },
            ScrollConfig {
                direction: ScrollAxis::Vertical,
                elastic: false,
                content_height: Fixed::from_int(ROW_H * ITEM_COUNT as i32),
                content_width: Fixed::ZERO,
            },
        ] {
            walk 0..POOL_SIZE with _i {
                row (
                    bg_color: ColorToken::SurfaceVariant,
                    text_color: ColorToken::OnSurface,
                    position: Position::Absolute,
                    left: 0,
                    top: 0,
                    width: 128,
                    height: ROW_H
                ) {}
            }
        }
    };
    let pool: Vec<Entity> = app
        .world
        .get::<Children>(list)
        .map(|c| c.0.clone())
        .unwrap_or_default();
    app.world.insert(list, LazyListPool::new(pool));

    // Tab B: Form — Switch + Slider + ProgressBar.
    mirui_macros::ui! {
        :(
            parent: root
            world: &mut app.world
        :)

        form_page (
            bg_color: ColorToken::Surface,
            width: 128,
            height: 114,
            direction: FlexDirection::Column,
            padding: Padding::all(10)
        ) [
            TabContent {
                tab_bar: tabs,
                index: 1,
            },
        ] {
            enable_row (
                direction: FlexDirection::Row,
                height: 28,
                align: AlignItems::Center
            ) {
                enable_label (text: "Enable", text_color: ColorToken::OnSurface, grow: 1.0) {}
                enable_switch (width: 40, height: 20) [
                    Switch::new(),
                    OffscreenRender::default(),
                ] {}
            }
            slider_row (
                height: 14,
                padding: Padding {
                    top: Dimension::px(6),
                    ..Default::default()
                }
            ) {
                value_slider (width: 108, height: 14) [
                    Slider::new(Fixed::ZERO, Fixed::from_int(100)),
                    FormSlider,
                ] {}
            }
            progress_row (
                height: 10,
                padding: Padding {
                    top: Dimension::px(8),
                    ..Default::default()
                }
            ) {
                value_progress (width: 108, height: 8, border_radius: 4) [
                    ProgressBar::new(),
                    FormProgress,
                ] {}
            }
        }
    };

    // Tab C: two colour blocks demonstrating builtin + custom tokens.
    // Both blocks repaint when theme_cycle_system rotates the
    // active Theme resource.
    mirui_macros::ui! {
        :(
            parent: root
            world: &mut app.world
        :)

        theme_page (
            bg_color: ColorToken::Surface,
            width: 128,
            height: 114,
            direction: FlexDirection::Column,
            padding: Padding::all(12),
            align: AlignItems::Center
        ) [
            TabContent {
                tab_bar: tabs,
                index: 2,
            },
        ] {
            primary_label (text: "Primary", text_color: ColorToken::OnSurface, height: 14) {}
            primary_block (width: 80, height: 18, bg_color: ColorToken::Primary, border_radius: 4) {}
            accent_label (
                text: "accent (custom)",
                text_color: ColorToken::OnSurfaceVariant,
                height: 12,
                padding: Padding {
                    top: Dimension::px(8),
                    ..Default::default()
                }
            ) {}
            accent_block (width: 80, height: 18, bg_color: ACCENT, border_radius: 4) {}
        }
    };

    let tab_kids = app
        .world
        .get::<Children>(tabs)
        .map(|c| c.0.clone())
        .unwrap_or_default();
    let (tab_list, tab_form, tab_theme) = (tab_kids[0], tab_kids[1], tab_kids[2]);
    let switch_e = *app
        .world
        .query::<Switch>()
        .collect()
        .first()
        .expect("form Switch must be installed");
    let slider_e = *app
        .world
        .query::<Slider>()
        .collect()
        .first()
        .expect("form Slider must be installed");

    let list_drag_anchor = list;

    app.world.insert_resource(
        SimTimeline::new(alloc::vec![
            SimAction::wait(800),
            SimAction::tap(DimPoint::CENTER).on(tab_form),
            SimAction::wait(800),
            SimAction::tap(DimPoint::CENTER).on(switch_e),
            SimAction::wait(800),
            SimAction::drag(
                DimPoint::percent(10, 50),
                DimPoint::percent(90, 50),
                600,
                ease::ease_in_out_cubic,
            )
            .on(slider_e),
            SimAction::wait(800),
            SimAction::tap(DimPoint::CENTER).on(switch_e),
            SimAction::wait(1500),
            SimAction::tap(DimPoint::CENTER).on(tab_theme),
            SimAction::wait(6500),
            SimAction::tap(DimPoint::CENTER).on(tab_list),
            SimAction::wait(800),
            SimAction::drag(
                DimPoint::percent(50, 80),
                DimPoint::percent(50, 20),
                300,
                ease::ease_in_out_cubic,
            )
            .on(list_drag_anchor),
            SimAction::wait(800),
            SimAction::drag(
                DimPoint::percent(50, 20),
                DimPoint::percent(50, 80),
                300,
                ease::ease_in_out_cubic,
            )
            .on(list_drag_anchor),
            SimAction::wait(800),
        ])
        .looping(true),
    );

    app.set_root(root);
}
