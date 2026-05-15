//! Layout (top → bottom):
//!   y= 0..14   TabBar (3 tabs)
//!   y=14..86   LazyList (8-slot pool over 50 rows, 12 px each)
//!   y=86..108  Slider track (108 wide) + Switch (right side)

use mirui::anim::{self, Tween, ease};
use mirui::app::App;
use mirui::components::lazy_list::{LazyList, LazyListBinder, LazyListPool, lazy_list_system};
use mirui::components::slider::Slider;
use mirui::components::switch::Switch;
use mirui::components::tabbar::TabBar;
use mirui::ecs::{Entity, World};
use mirui::event::GestureHandler;
use mirui::event::gesture::GestureEvent;
use mirui::event::scroll::{ScrollAxis, ScrollConfig, ScrollOffset};
use mirui::event::sim::{SimAction, SimTimeline, sim_timeline_system};
use mirui::layout::*;
use mirui::types::{Color, Dimension, Fixed, Point};
use mirui::widget::Text;
use mirui::widget::builder::WidgetBuilder;
use mirui::widget::dirty::Dirty;

use alloc::format;
use alloc::vec::Vec;

const ROW_H: i32 = 12;
const POOL_SIZE: usize = 8;
const ITEM_COUNT: u32 = 50;

mirui_macros::animate!(AnimateTabIndicator, |world, entity, value| {
    if let Some(tb) = world.get_mut::<TabBar>(entity) {
        tb.indicator_offset = value;
    }
    world.insert(entity, Dirty);
});

mirui_macros::animate!(AnimateThumbX, |world, entity, value| {
    mirui::widget::set_position(world, entity, value, Fixed::from_int(2));
});

struct LastTab(u8);

fn observe_tabbar(world: &mut World) {
    let tabbars: Vec<_> = world.query::<TabBar>().collect();
    for entity in tabbars {
        let current = match world.get::<TabBar>(entity) {
            Some(tb) => tb.selected,
            None => continue,
        };
        let from = world
            .get::<LastTab>(entity)
            .map(|s| s.0)
            .unwrap_or(current);
        if from == current {
            continue;
        }
        let from_offset = Fixed::from_int(from as i32);
        let to_offset = Fixed::from_int(current as i32);
        world.insert(entity, LastTab(current));
        world.insert(
            entity,
            AnimateTabIndicator(Tween::ease_to(from_offset, to_offset, 220).into()),
        );
    }
}

fn slider_handler(world: &mut World, entity: Entity, event: &GestureEvent) -> bool {
    let x = match event {
        GestureEvent::DragMove { x, .. } | GestureEvent::Tap { x, .. } => *x,
        _ => return false,
    };
    let track_x = world
        .get::<mirui::widget::ComputedRect>(entity)
        .map(|r| r.0.x)
        .unwrap_or(Fixed::ZERO);
    let track_w = Fixed::from_int(108);
    let local_x = x - track_x;
    let ratio = local_x / track_w;
    let clamped = {
        let Some(slider) = world.get_mut::<Slider>(entity) else {
            return false;
        };
        slider.set_ratio(ratio);
        slider.ratio()
    };
    let fill_w = clamped * track_w;
    if let Some(children) = world.get::<mirui::widget::Children>(entity) {
        let cc: Vec<Entity> = children.0.clone();
        if cc.len() >= 2 {
            let fill_mask = cc[0];
            let thumb_entity = cc[1];
            if let Some(style) = world.get_mut::<mirui::widget::Style>(fill_mask) {
                style.layout.width = Dimension::Px(fill_w);
            }
            world.insert(fill_mask, Dirty);
            let thumb_w = Fixed::from_int(10);
            let thumb_x = clamped * (track_w - thumb_w);
            mirui::widget::set_position(world, thumb_entity, thumb_x, Fixed::from_int(0));
        }
    }
    world.insert(entity, Dirty);
    true
}

fn switch_handler(world: &mut World, entity: Entity, event: &GestureEvent) -> bool {
    if !matches!(event, GestureEvent::Tap { .. }) {
        return false;
    }
    let is_on = {
        let Some(sw) = world.get_mut::<Switch>(entity) else {
            return false;
        };
        sw.toggle();
        sw.on
    };
    if let Some(style) = world.get_mut::<mirui::widget::Style>(entity) {
        style.bg_color = Some(if is_on {
            Color::rgb(63, 185, 80)
        } else {
            Color::rgb(80, 80, 100)
        });
    }
    world.insert(entity, Dirty);

    if let Some(children) = world.get::<mirui::widget::Children>(entity) {
        if let Some(&thumb) = children.0.first() {
            let target_x = if is_on {
                Fixed::from_int(15)
            } else {
                Fixed::from_int(2)
            };
            let current_x = world
                .get::<mirui::widget::Style>(thumb)
                .and_then(|s| match s.layout.left {
                    Dimension::Px(p) => Some(p),
                    _ => None,
                })
                .unwrap_or(Fixed::from_int(2));
            world.insert(
                thumb,
                AnimateThumbX(Tween::ease_to(current_x, target_x, 200).into()),
            );
        }
    }
    true
}

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
    app.add_system(AnimateThumbX::system());
    app.add_system(AnimateTabIndicator::system());
    app.add_system(observe_tabbar);
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
        ) {
            tab0 (
                text: "A",
                text_color: Color::rgb(220, 220, 230),
                grow: 1.0,
                align: AlignItems::Center,
                justify: JustifyContent::Center
            ) {}
            tab1 (
                text: "B",
                text_color: Color::rgb(220, 220, 230),
                grow: 1.0,
                align: AlignItems::Center,
                justify: JustifyContent::Center
            ) {}
            tab2 (
                text: "C",
                text_color: Color::rgb(220, 220, 230),
                grow: 1.0,
                align: AlignItems::Center,
                justify: JustifyContent::Center
            ) {}
        }
    };
    app.world.insert(
        tabs,
        TabBar::new(3).with_indicator(Color::rgb(88, 166, 255), 2),
    );

    let list = WidgetBuilder::new(&mut app.world)
        .bg_color(Color::rgb(28, 28, 40))
        .layout(LayoutStyle {
            width: Dimension::px(128),
            height: Dimension::px(72),
            ..Default::default()
        })
        .id();
    app.world.insert(list, mirui::widget::Parent(root));
    if let Some(c) = app.world.get_mut::<mirui::widget::Children>(root) {
        c.0.push(list);
    }

    let mut pool: Vec<Entity> = Vec::with_capacity(POOL_SIZE);
    for _ in 0..POOL_SIZE {
        let e = WidgetBuilder::new(&mut app.world)
            .bg_color(Color::rgb(40, 40, 56))
            .text_color(Color::rgb(220, 220, 230))
            .layout(LayoutStyle {
                position: Position::Absolute,
                left: Dimension::Px(Fixed::ZERO),
                top: Dimension::Px(Fixed::ZERO),
                width: Dimension::px(128),
                height: Dimension::px(ROW_H),
                ..Default::default()
            })
            .id();
        app.world.insert(e, mirui::widget::Parent(list));
        pool.push(e);
    }
    app.world
        .insert(list, mirui::widget::Children(pool.clone()));
    app.world
        .insert(list, LazyList::new(ITEM_COUNT, ROW_H, POOL_SIZE as u8));
    app.world.insert(list, LazyListPool::new(pool));
    app.world.insert(list, LazyListBinder { bind: row_binder });
    app.world.insert(
        list,
        ScrollOffset {
            x: Fixed::ZERO,
            y: Fixed::ZERO,
        },
    );
    app.world.insert(
        list,
        ScrollConfig {
            direction: ScrollAxis::Vertical,
            elastic: false,
            content_height: Fixed::from_int(ROW_H * ITEM_COUNT as i32),
            content_width: Fixed::ZERO,
        },
    );

    mirui_macros::ui! {
        :(
            parent: root
            world: &mut app.world
        :)

        slider_track (
            bg_color: Color::rgb(60, 60, 80),
            position: Position::Absolute,
            left: 0,
            top: 90,
            width: 108,
            height: 8,
            border_radius: 4
        ) [
            Slider::new(Fixed::ZERO, Fixed::from_int(100)),
            GestureHandler { on_gesture: slider_handler }
        ] {
            fill_mask (
                width: 54,
                height: 8,
                clip_children: true
            ) {
                fill_inner (
                    bg_color: Color::rgb(88, 166, 255),
                    position: Position::Absolute,
                    left: 0,
                    top: 0,
                    width: 108,
                    height: 8,
                    border_radius: 4
                ) {}
            }
            thumb (
                bg_color: Color::rgb(255, 255, 255),
                position: Position::Absolute,
                left: 49,
                top: 0,
                width: 8,
                height: 8,
                border_radius: 4
            ) {}
        }
    };
    mirui_macros::ui! {
        :(
            parent: root
            world: &mut app.world
        :)

        switch_track (
            bg_color: Color::rgb(80, 80, 100),
            position: Position::Absolute,
            left: 110,
            top: 88,
            width: 26,
            height: 12,
            border_radius: 6
        ) [
            Switch::new(),
            GestureHandler { on_gesture: switch_handler }
        ] {
            sw_thumb (
                bg_color: Color::rgb(255, 255, 255),
                position: Position::Absolute,
                left: 2,
                top: 2,
                width: 8,
                height: 8,
                border_radius: 4
            ) {}
        }
    };

    app.world.insert_resource(
        SimTimeline::new(alloc::vec![
            SimAction::Wait(500),
            SimAction::Tap(Point::new(20, 7)),  // tab A
            SimAction::Wait(800),
            SimAction::Tap(Point::new(64, 7)),  // tab B
            SimAction::Wait(800),
            SimAction::Tap(Point::new(108, 7)), // tab C
            SimAction::Wait(800),
            SimAction::Drag {
                from: Point::new(2, 94),
                to: Point::new(120, 94),
                duration_ms: 600,
                ease: ease::ease_in_out_cubic,
            },
            SimAction::Wait(400),
            SimAction::Drag {
                from: Point::new(120, 94),
                to: Point::new(2, 94),
                duration_ms: 600,
                ease: ease::ease_in_out_cubic,
            },
            SimAction::Wait(400),
            SimAction::Drag {
                from: Point::new(64, 80),
                to: Point::new(64, 16),
                duration_ms: 700,
                ease: ease::ease_out_cubic,
            },
            SimAction::Wait(800),
            SimAction::Drag {
                from: Point::new(64, 16),
                to: Point::new(64, 80),
                duration_ms: 700,
                ease: ease::ease_out_cubic,
            },
            SimAction::Wait(400),
            SimAction::Tap(Point::new(123, 94)), // toggle switch
            SimAction::Wait(800),
            SimAction::Tap(Point::new(123, 94)),
            SimAction::Wait(1500),
        ])
        .looping(true),
    );

    app.set_root(root);
}
