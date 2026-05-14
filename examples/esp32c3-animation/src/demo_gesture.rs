use alloc::vec::Vec;
use mirui::anim::{self, ease, Animation, FrameClock, PlayMode};
use mirui::app::App;
use mirui::components::slider::Slider;
use mirui::components::switch::Switch;
use mirui::ecs::{Entity, World};
use mirui::event::GestureHandler;
use mirui::event::gesture::{GestureEvent, GestureSystem};
use mirui::event::hit_test::hit_test;
use mirui::event::input::InputEvent;
use mirui::layout::*;
use mirui::types::{Color, Dimension, Fixed};
use mirui::widget::builder::WidgetBuilder;
use mirui::widget::dirty::Dirty;

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
        .unwrap_or(Fixed::from_int(80));
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

struct SimState {
    phase: u8,
    last_tick: u32,
    step: u8,
    start_tick: u32,
}

struct SimRoot(Entity);
struct SimSlider(Entity);
struct SimSwitch(Entity);

const SIM_INTERVAL: u32 = 160_000_000 / 120;
const SIM_STEPS: u8 = 60;

fn sim_input_system(world: &mut World) {
    let now = systimer_now();
    let (phase, step) = {
        let Some(sim) = world.resource_mut::<SimState>() else {
            return;
        };
        if now.wrapping_sub(sim.last_tick) < SIM_INTERVAL {
            return;
        }
        sim.last_tick = now;
        sim.step += 1;
        if sim.step >= SIM_STEPS {
            sim.step = 0;
            sim.phase = (sim.phase + 1) % 3;
        }
        (sim.phase, sim.step)
    };

    let (lw, lh) = world
        .resource::<mirui::surface::DisplayInfo>()
        .map(|d| (d.width, d.height))
        .unwrap_or((128, 128));
    let root = world
        .resource::<SimRoot>()
        .map(|r| r.0)
        .unwrap_or(Entity { id: 0, generation: 0 });
    let start_tick = world
        .resource::<SimState>()
        .map(|s| s.start_tick)
        .unwrap_or(0);
    let now_ms = now.wrapping_sub(start_tick) / 160_000;

    match phase {
        0 | 2 => {
            let slider_x = world
                .resource::<SimSlider>()
                .and_then(|s| world.get::<mirui::widget::ComputedRect>(s.0))
                .map(|r| r.0.x)
                .unwrap_or(Fixed::from_int(10));
            let y = Fixed::from_int(18);
            let (from, to) = if phase == 0 { (5, 100) } else { (100, 5) };
            if step == 0 {
                let x = slider_x + Fixed::from_int(from);
                let hit = hit_test(world, root, x, y, lw, lh);
                if let Some(gs) = world.resource_mut::<GestureSystem>() {
                    gs.recognizer
                        .update(&InputEvent::PointerDown { id: 0, x, y }, now_ms, hit, &mut gs.events);
                }
            } else if step < SIM_STEPS - 1 {
                let t = Fixed::from_raw(
                    (step as i32) * Fixed::ONE.raw() / ((SIM_STEPS - 2) as i32),
                );
                let eased = ease::ease_in_out_cubic(t);
                let x_offset = Fixed::from_int(from)
                    + eased * Fixed::from_int(to - from);
                let x = slider_x + x_offset;
                if let Some(gs) = world.resource_mut::<GestureSystem>() {
                    gs.recognizer
                        .update(&InputEvent::PointerMove { id: 0, x, y }, now_ms, None, &mut gs.events);
                }
            } else {
                let x = slider_x + Fixed::from_int(to);
                if let Some(gs) = world.resource_mut::<GestureSystem>() {
                    gs.recognizer
                        .update(&InputEvent::PointerUp { id: 0, x, y }, now_ms, None, &mut gs.events);
                }
            }
        }
        1 => {
            let half = SIM_STEPS / 2;
            if step == 0 || step == half {
                let switch_center = world
                    .resource::<SimSwitch>()
                    .and_then(|s| world.get::<mirui::widget::ComputedRect>(s.0))
                    .map(|r| Fixed::from_int((r.0.x.to_int() + r.0.w.to_int() / 2).max(0)))
                    .unwrap_or(Fixed::from_int(100));
                let y = Fixed::from_int(50);
                let hit = hit_test(world, root, switch_center, y, lw, lh);
                if let Some(gs) = world.resource_mut::<GestureSystem>() {
                    gs.recognizer.update(
                        &InputEvent::PointerDown { id: 0, x: switch_center, y },
                        now_ms,
                        hit,
                        &mut gs.events,
                    );
                }
            } else if step == 1 || step == half + 1 {
                let x = Fixed::from_int(100);
                let y = Fixed::from_int(50);
                if let Some(gs) = world.resource_mut::<GestureSystem>() {
                    gs.recognizer.update(
                        &InputEvent::PointerUp { id: 0, x, y },
                        now_ms,
                        None,
                        &mut gs.events,
                    );
                }
            }
        }
        _ => {}
    }

    let pending: Vec<GestureEvent> = world
        .resource_mut::<GestureSystem>()
        .map(|gs| gs.events.drain().collect())
        .unwrap_or_default();
    for gesture in &pending {
        mirui::event::bubble_dispatch(world, gesture);
    }
}

fn anim_clock() -> u64 {
    (systimer_now() as u64).saturating_mul(1000) / 160
}

pub fn setup<B: mirui::surface::FramebufferAccess>(app: &mut App<B>) {
    app.world.insert_resource(FrameClock::new(anim_clock));
    app.add_system(anim::sync_delta_time_ms);
    app.add_system(AnimateThumbX::system());
    app.add_system(sim_input_system);

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

    app.world.insert_resource(SimState {
        phase: 0,
        last_tick: systimer_now(),
        step: 0,
        start_tick: systimer_now(),
    });
    app.world.insert_resource(SimRoot(root));
    app.world.insert_resource(SimSlider(slider_track));
    app.world.insert_resource(SimSwitch(switch_track));

    app.set_root(root);
}
