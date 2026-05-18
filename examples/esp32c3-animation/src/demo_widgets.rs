//! ESP showcase exercising mirui v0.14 ThemedColor:
//! - tab "List" → LazyList of 50 rows; rows use `Surface` /
//!   `OnSurface` tokens.
//! - tab "Form" → Switch + Slider + ProgressBar with the Slider
//!   value pushed onto the ProgressBar by `slider_to_progress_system`.
//!   Every widget colour comes from its built-in default token.
//! - tab "Theme" → two colour blocks (one for `Primary`, one for a
//!   user-defined `accent` token). `theme_cycle_system` rotates
//!   Dark / Light / Custom every 3 s; the whole UI repaints in the
//!   new palette on the next frame.

use mirui::anim::{self, ease};
use mirui::app::App;
use mirui::components::lazy_list::{LazyList, LazyListBinder, LazyListPool, lazy_list_system};
use mirui::components::progress_bar::ProgressBar;
use mirui::components::slider::Slider;
use mirui::components::switch::Switch;
use mirui::components::tab_pages::TabContent;
use mirui::components::tabbar::TabBar;
use mirui::components::text::Text;
use mirui::ecs::{Entity, MonoClock, World};
use mirui::event::scroll::{ScrollAxis, ScrollConfig, ScrollOffset};
use mirui::event::sim::{SimAction, SimTimeline, sim_timeline_system};
use mirui::layout::*;
use mirui::types::{Color, Dimension, Fixed, Point};
use mirui::widget::builder::WidgetBuilder;
use mirui::widget::dirty::Dirty;
use mirui::widget::{Children, ColorToken, Theme};

use alloc::format;
use alloc::vec::Vec;

const ROW_H: i32 = 12;
const POOL_SIZE: usize = 8;
const ITEM_COUNT: u32 = 50;

/// User-defined token. The "Theme" tab's lower colour block reads
/// it; each preset theme below binds it to a different colour.
const ACCENT: ColorToken = ColorToken::custom("accent");

/// `slider_to_progress_system` reads `FormSlider`, writes `FormProgress`.
struct FormSlider;
struct FormProgress;

/// State for `theme_cycle_system`: when to swap next, and which preset to swap to.
struct ThemeCycle {
    next_at_ms: u64,
    index: u8,
}

fn dark_with_accent() -> Theme {
    let mut t = Theme::dark();
    t.set(ACCENT, Color::rgb(255, 200, 60));
    t
}

fn light_with_accent() -> Theme {
    let mut t = Theme::light();
    t.set(ACCENT, Color::rgb(220, 60, 90));
    t
}

fn custom_theme() -> Theme {
    let mut t = Theme::dark();
    t.set(ColorToken::Primary, Color::rgb(255, 105, 180))
        .set(ColorToken::OnPrimary, Color::rgb(20, 20, 30))
        .set(ColorToken::Success, Color::rgb(255, 200, 60))
        .set(ColorToken::Surface, Color::rgb(38, 28, 50))
        .set(ColorToken::SurfaceVariant, Color::rgb(70, 50, 90))
        .set(ColorToken::OnSurface, Color::rgb(245, 235, 255))
        .set(ColorToken::OnSurfaceVariant, Color::rgb(180, 150, 200))
        .set(ACCENT, Color::rgb(140, 200, 220));
    t
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

/// Cycle Theme every 3 s. The swap itself is one
/// `world.insert_resource(theme)` call — Theme is a World resource
/// like any other. The recursive `Dirty` walk is what tells the
/// renderer to repaint widgets reading `ColorToken::*`.
fn theme_cycle_system(world: &mut World) {
    let now_ms = match world.resource::<MonoClock>() {
        Some(c) => c.now_ms() as u64,
        None => return,
    };
    let mut should_swap = None;
    if let Some(cycle) = world.resource_mut::<ThemeCycle>() {
        if now_ms >= cycle.next_at_ms {
            cycle.next_at_ms = now_ms + 3_000;
            cycle.index = (cycle.index + 1) % 3;
            should_swap = Some(cycle.index);
        }
    }
    let Some(idx) = should_swap else { return };
    let theme = match idx {
        0 => dark_with_accent(),
        1 => light_with_accent(),
        _ => custom_theme(),
    };
    world.insert_resource(theme);
    let roots: Vec<Entity> = world.query::<Children>().collect();
    for r in roots {
        mark_subtree_dirty(world, r);
    }
}

fn mark_subtree_dirty(world: &mut World, entity: Entity) {
    world.insert(entity, Dirty);
    let children = world
        .get::<Children>(entity)
        .map(|c| c.0.clone())
        .unwrap_or_default();
    for c in children {
        mark_subtree_dirty(world, c);
    }
}

pub fn setup<B: mirui::surface::FramebufferAccess>(app: &mut App<B>) {
    app.add_system(anim::sync_delta_time_ms);
    app.add_system(lazy_list_system);
    app.add_system(sim_timeline_system);
    app.add_system(slider_to_progress_system);
    app.add_system(theme_cycle_system);

    // Start with the dark palette + accent token bound.
    app.world.insert_resource(dark_with_accent());
    app.world.insert_resource(ThemeCycle {
        next_at_ms: 3_000,
        index: 0,
    });

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
        :( parent: root world: &mut app.world :)
        tabs (
            bg_color: ColorToken::SurfaceVariant,
            width: 128, height: 14
        ) [
            TabBar::new(3).with_indicator_height(2)
        ] {
            tab0 (text: "List", text_color: ColorToken::OnSurface,
                grow: 1.0, align: AlignItems::Center, justify: JustifyContent::Center) {}
            tab1 (text: "Form", text_color: ColorToken::OnSurface,
                grow: 1.0, align: AlignItems::Center, justify: JustifyContent::Center) {}
            tab2 (text: "Thm", text_color: ColorToken::OnSurface,
                grow: 1.0, align: AlignItems::Center, justify: JustifyContent::Center) {}
        }
    };

    // Tab A: LazyList of 50 rows.
    let list = mirui_macros::ui! {
        :( parent: root world: &mut app.world :)
        list (
            bg_color: ColorToken::Surface,
            width: 128, height: 114
        ) [
            TabContent { tab_bar: tabs, index: 0 },
            LazyList::new(ITEM_COUNT, ROW_H, POOL_SIZE as u8),
            LazyListBinder { bind: row_binder },
            ScrollOffset { x: Fixed::ZERO, y: Fixed::ZERO },
            ScrollConfig {
                direction: ScrollAxis::Vertical,
                elastic: false,
                content_height: Fixed::from_int(ROW_H * ITEM_COUNT as i32),
                content_width: Fixed::ZERO,
            }
        ] {
            walk 0..POOL_SIZE with _i {
                row (
                    bg_color: ColorToken::SurfaceVariant,
                    text_color: ColorToken::OnSurface,
                    position: Position::Absolute,
                    left: 0, top: 0,
                    width: 128, height: ROW_H
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
        :( parent: root world: &mut app.world :)
        form_page (
            bg_color: ColorToken::Surface,
            width: 128, height: 114,
            direction: FlexDirection::Column,
            padding: Padding::all(10)
        ) [ TabContent { tab_bar: tabs, index: 1 } ] {
            enable_row (
                direction: FlexDirection::Row,
                height: 28, align: AlignItems::Center
            ) {
                enable_label (text: "Enable", text_color: ColorToken::OnSurface, grow: 1.0) {}
                enable_switch (width: 40, height: 20) [ Switch::new() ] {}
            }
            slider_row (height: 14, padding: Padding { top: Dimension::px(6), ..Default::default() }) {
                value_slider (width: 108, height: 14) [
                    Slider::new(Fixed::ZERO, Fixed::from_int(100)),
                    FormSlider,
                ] {}
            }
            progress_row (height: 10, padding: Padding { top: Dimension::px(8), ..Default::default() }) {
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
        :( parent: root world: &mut app.world :)
        theme_page (
            bg_color: ColorToken::Surface,
            width: 128, height: 114,
            direction: FlexDirection::Column,
            padding: Padding::all(12),
            align: AlignItems::Center
        ) [ TabContent { tab_bar: tabs, index: 2 } ] {
            primary_label (text: "Primary", text_color: ColorToken::OnSurface, height: 14) {}
            primary_block (width: 80, height: 18, bg_color: ColorToken::Primary, border_radius: 4) {}
            accent_label (
                text: "accent (custom)",
                text_color: ColorToken::OnSurfaceVariant,
                height: 12, padding: Padding { top: Dimension::px(8), ..Default::default() }
            ) {}
            accent_block (width: 80, height: 18, bg_color: ACCENT, border_radius: 4) {}
        }
    };

    // Sim playback: walk through the three tabs and exercise each.
    // Theme cycles independently every 3 s via theme_cycle_system.
    app.world.insert_resource(
        SimTimeline::new(alloc::vec![
            // Form tab: toggle Switch on, drag Slider, watch Progress fill.
            SimAction::Wait(500),
            SimAction::Tap(Point::new(64, 7)),  // Form
            SimAction::Wait(800),
            SimAction::Tap(Point::new(105, 28)), // Switch on
            SimAction::Wait(800),
            SimAction::Drag {
                from: Point::new(14, 60),
                to: Point::new(116, 60),
                duration_ms: 600,
                ease: ease::ease_in_out_cubic,
            },
            SimAction::Wait(800),
            SimAction::Tap(Point::new(105, 28)), // Switch off (disabled look)
            SimAction::Wait(1500),
            // Theme tab: just sit there while theme_cycle_system rotates.
            SimAction::Tap(Point::new(108, 7)),
            SimAction::Wait(6500),
            // List tab: scroll up/down.
            SimAction::Tap(Point::new(20, 7)),
            SimAction::Wait(800),
            SimAction::Drag {
                from: Point::new(64, 100),
                to: Point::new(64, 30),
                duration_ms: 700,
                ease: ease::ease_in_out_cubic,
            },
            SimAction::Wait(800),
            SimAction::Drag {
                from: Point::new(64, 30),
                to: Point::new(64, 100),
                duration_ms: 700,
                ease: ease::ease_in_out_cubic,
            },
            SimAction::Wait(800),
        ])
        .looping(true),
    );

    app.set_root(root);
}
