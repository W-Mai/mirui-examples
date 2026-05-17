//! Three-tab showcase on the 128×128 ESP32-C3 LCD:
//!   tab A → LazyList (50 rows of 12 px)
//!   tab B → Slider
//!   tab C → Switch
//! Tab indicator slide is mirui's built-in tab_pages_system.
//! Slider and Switch render + interact via mirui's built-in views.

use mirui::anim::{self, ease};
use mirui::app::App;
use mirui::components::lazy_list::{LazyList, LazyListBinder, LazyListPool, lazy_list_system};
use mirui::components::slider::Slider;
use mirui::components::switch::Switch;
use mirui::components::tab_pages::TabContent;
use mirui::components::tabbar::TabBar;
use mirui::components::text::Text;
use mirui::ecs::{Entity, World};
use mirui::event::scroll::{ScrollAxis, ScrollConfig, ScrollOffset};
use mirui::event::sim::{SimAction, SimTimeline, sim_timeline_system};
use mirui::layout::*;
use mirui::types::{Color, Dimension, Fixed, Point};
use mirui::widget::builder::WidgetBuilder;

use alloc::format;
use alloc::vec::Vec;

const ROW_H: i32 = 12;
const POOL_SIZE: usize = 8;
const ITEM_COUNT: u32 = 50;

fn row_binder(world: &mut World, entity: Entity, index: u32) {
    let label = format!("Row {index}");
    if let Some(t) = world.get_mut::<Text>(entity) {
        t.0 = label.into_bytes();
    } else {
        world.insert(entity, Text(label.into_bytes()));
    }
}

pub fn setup<B: mirui::surface::FramebufferAccess>(app: &mut App<B>) {
    app.add_system(anim::sync_delta_time_ms);
    app.add_system(lazy_list_system);
    app.add_system(sim_timeline_system);

    let root = WidgetBuilder::new(&mut app.world)
        .bg_color(Color::rgb(30, 30, 46))
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
            bg_color: Color::rgb(40, 40, 56),
            width: 128,
            height: 14
        ) [
            TabBar::new(3).with_indicator_height(2)
        ] {
            tab0 ( text: "List", text_color: Color::rgb(220, 220, 230),
                grow: 1.0, align: AlignItems::Center, justify: JustifyContent::Center ) {}
            tab1 ( text: "Slide", text_color: Color::rgb(220, 220, 230),
                grow: 1.0, align: AlignItems::Center, justify: JustifyContent::Center ) {}
            tab2 ( text: "Sw", text_color: Color::rgb(220, 220, 230),
                grow: 1.0, align: AlignItems::Center, justify: JustifyContent::Center ) {}
        }
    };

    // Tab A: LazyList (50 rows). Pool spawned via walk; bound after.
    let list = mirui_macros::ui! {
        :(
            parent: root
            world: &mut app.world
        :)

        list (
            bg_color: Color::rgb(28, 28, 40),
            width: 128,
            height: 114
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
                    bg_color: Color::rgb(40, 40, 56),
                    text_color: Color::rgb(220, 220, 230),
                    position: Position::Absolute,
                    left: 0, top: 0,
                    width: 128, height: ROW_H
                ) {}
            }
        }
    };
    let pool: Vec<Entity> = app
        .world
        .get::<mirui::widget::Children>(list)
        .map(|c| c.0.clone())
        .unwrap_or_default();
    app.world.insert(list, LazyListPool::new(pool));

    // Tab B: Slider centered in the page.
    mirui_macros::ui! {
        :(
            parent: root
            world: &mut app.world
        :)

        slide_page (
            bg_color: Color::rgb(28, 28, 40),
            width: 128,
            height: 114,
            align: AlignItems::Center,
            justify: JustifyContent::Center
        ) [
            TabContent { tab_bar: tabs, index: 1 }
        ] {
            slider (width: 108, height: 12) [
                Slider::new(Fixed::ZERO, Fixed::from_int(100)),
            ] {}
        }
    };

    // Tab C: Switch centered.
    mirui_macros::ui! {
        :(
            parent: root
            world: &mut app.world
        :)

        switch_page (
            bg_color: Color::rgb(28, 28, 40),
            width: 128,
            height: 114,
            align: AlignItems::Center,
            justify: JustifyContent::Center
        ) [
            TabContent { tab_bar: tabs, index: 2 }
        ] {
            sw (width: 50, height: 26) [ Switch::new() ] {}
        }
    };

    // Sim playback: tap each tab so all three content pages get exercised
    // over a single capture window. Coordinates land on the bar at y=7.
    app.world.insert_resource(
        SimTimeline::new(alloc::vec![
            SimAction::Wait(500),
            SimAction::Tap(Point::new(64, 7)),  // Slide
            SimAction::Wait(1500),
            SimAction::Drag {
                from: Point::new(14, 71),
                to: Point::new(116, 71),
                duration_ms: 600,
                ease: ease::ease_in_out_cubic,
            },
            SimAction::Wait(800),
            SimAction::Tap(Point::new(108, 7)), // Sw
            SimAction::Wait(1200),
            SimAction::Tap(Point::new(64, 60)), // toggle switch
            SimAction::Wait(800),
            SimAction::Tap(Point::new(64, 60)),
            SimAction::Wait(800),
            SimAction::Tap(Point::new(20, 7)),  // List
            SimAction::Wait(800),
            // Scroll the list down then back up.
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
