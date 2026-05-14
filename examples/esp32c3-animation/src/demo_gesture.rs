use mirui::anim::{self, ease, Animation, FrameClock, PlayMode};
use mirui::app::App;
use mirui::components::slider::Slider;
use mirui::components::switch::Switch;
use mirui::ecs::{Entity, World};
use mirui::event::GestureHandler;
use mirui::event::gesture::GestureEvent;
use mirui::event::sim::{SimAction, SimTimeline, sim_timeline_system};
use mirui::layout::*;
use mirui::types::{Color, Dimension, Fixed, Point};
use mirui::widget::builder::WidgetBuilder;
use mirui::widget::dirty::Dirty;

use alloc::vec::Vec;

use crate::board::systimer_now;

mirui_macros::animation!(AnimateThumbX, |world, entity, value| {
    mirui::widget::set_position(world, entity, value, Fixed::from_int(2));
});

struct SliderTrackWidth(Fixed);

fn slider_handler(world: &mut World, entity: Entity, event: &GestureEvent) -> bool {
    let x = match event {
        GestureEvent::DragMove { x, .. } | GestureEvent::Tap { x, .. } => *x,
        _ => return false,
    };
    let track_w = world
        .get::<SliderTrackWidth>(entity)
        .map(|t| t.0)
        .unwrap_or(Fixed::from_int(108));
    let track_x = world
        .get::<mirui::widget::ComputedRect>(entity)
        .map(|r| r.0.x)
        .unwrap_or(Fixed::ZERO);
    let local_x = x - track_x;
    let ratio = local_x / track_w;
    if let Some(slider) = world.get_mut::<Slider>(entity) {
        slider.set_ratio(ratio);
        let fill_w = slider.ratio() * track_w;
        let fill_color = slider.fill_color;
        if let Some(children) = world.get::<mirui::widget::Children>(entity) {
            let cc: Vec<Entity> = children.0.clone();
            if cc.len() >= 2 {
                if let Some(style) = world.get_mut::<mirui::widget::Style>(cc[0]) {
                    style.layout.width = Dimension::Px(fill_w);
                    style.bg_color = Some(fill_color);
                }
                world.insert(cc[0], Dirty);
                let thumb_x = (fill_w - Fixed::from_int(5)).max(Fixed::ZERO);
                mirui::widget::set_position(world, cc[1], thumb_x, Fixed::from_int(0));
            }
        }
    }
    world.insert(entity, Dirty);
    true
}

fn switch_handler(world: &mut World, entity: Entity, event: &GestureEvent) -> bool {
    if !matches!(event, GestureEvent::Tap { .. }) {
        return false;
    }
    let (is_on, track_color) = {
        let Some(sw) = world.get_mut::<Switch>(entity) else {
            return false;
        };
        sw.toggle();
        (sw.on, sw.track_color())
    };
    if let Some(style) = world.get_mut::<mirui::widget::Style>(entity) {
        style.bg_color = Some(track_color);
    }
    world.insert(entity, Dirty);

    if let Some(children) = world.get::<mirui::widget::Children>(entity) {
        if let Some(&thumb) = children.0.first() {
            let target_x = if is_on {
                Fixed::from_int(16)
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
                AnimateThumbX(Animation::new(
                    current_x,
                    target_x,
                    200,
                    ease::ease_out_cubic,
                    PlayMode::Once,
                )),
            );
        }
    }
    true
}

fn anim_clock() -> u64 {
    (systimer_now() as u64).saturating_mul(1000) / 160
}

pub fn setup<B: mirui::surface::FramebufferAccess>(app: &mut App<B>) {
    app.world.insert_resource(FrameClock::new(anim_clock));
    app.add_system(anim::sync_delta_time_ms);
    app.add_system(AnimateThumbX::system());
    app.add_system(sim_timeline_system);

    let root = WidgetBuilder::new(&mut app.world)
        .bg_color(Color::rgb(30, 30, 46))
        .layout(LayoutStyle {
            direction: FlexDirection::Column,
            width: Dimension::px(128),
            height: Dimension::px(128),
            padding: Padding {
                top: Dimension::px(10),
                left: Dimension::px(10),
                right: Dimension::px(10),
                bottom: Dimension::px(10),
            },
            ..Default::default()
        })
        .id();

    let slider_track = mirui_macros::ui! {
        :(
            parent: root
            world: &mut app.world
        :)

        slider_track (
            bg_color: Color::rgb(60, 60, 80),
            width: 108,
            height: 10,
            border_radius: 5
        ) {
            fill (
                bg_color: Color::rgb(88, 166, 255),
                width: 54,
                height: 10,
                border_radius: 5
            ) {}
            thumb (
                bg_color: Color::rgb(255, 255, 255),
                position: Position::Absolute,
                left: 49,
                top: 0,
                width: 10,
                height: 10,
                border_radius: 5
            ) {}
        }
    };
    app.world
        .insert(slider_track, Slider::new(Fixed::ZERO, Fixed::from_int(100)));
    app.world
        .insert(slider_track, SliderTrackWidth(Fixed::from_int(108)));
    app.world.insert(
        slider_track,
        GestureHandler {
            on_gesture: slider_handler,
        },
    );

    let switch_track = mirui_macros::ui! {
        :(
            parent: root
            world: &mut app.world
        :)

        switch_track (
            bg_color: Color::rgb(80, 80, 100),
            position: Position::Absolute,
            left: 10,
            top: 40,
            width: 34,
            height: 18,
            border_radius: 9
        ) {
            sw_thumb (
                bg_color: Color::rgb(255, 255, 255),
                position: Position::Absolute,
                left: 2,
                top: 2,
                width: 14,
                height: 14,
                border_radius: 7
            ) {}
        }
    };
    app.world.insert(switch_track, Switch::new());
    app.world.insert(
        switch_track,
        GestureHandler {
            on_gesture: switch_handler,
        },
    );

    app.world.insert_resource(
        SimTimeline::new(alloc::vec![
            SimAction::Drag {
                from: Point::new(15, 15),
                to: Point::new(115, 15),
                duration_ms: 500,
                ease: ease::ease_in_out_cubic,
            },
            SimAction::Wait(300),
            SimAction::Tap(Point::new(27, 50)),
            SimAction::Wait(400),
            SimAction::Drag {
                from: Point::new(115, 15),
                to: Point::new(15, 15),
                duration_ms: 500,
                ease: ease::ease_in_out_cubic,
            },
            SimAction::Wait(300),
            SimAction::Tap(Point::new(27, 50)),
            SimAction::Wait(400),
        ])
        .looping(true),
    );

    app.set_root(root);
}
