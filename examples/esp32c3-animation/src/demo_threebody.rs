use alloc::vec::Vec;
use mirui::app::App;
use mirui::components::assets::*;
use mirui::components::image::Image;
use mirui::ecs::World;
use mirui::layout::*;
use mirui::types::{Color, Dimension, Fixed};
use mirui::widget::builder::WidgetBuilder;

use crate::board::{systimer_now, H, W};

struct Velocity { vx: Fixed, vy: Fixed }
struct PhysicsBody { x: Fixed, y: Fixed }
struct PhysicsTime { last_tick: u32, accumulator: u32 }

const PHYSICS_DT: u32 = 1_111_111;

fn isqrt(n: u32) -> u32 {
    if n == 0 { return 0; }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x { x = y; y = (x + n / x) / 2; }
    x
}

fn physics_tick_system(world: &mut World) {
    let now = systimer_now();
    let (steps,) = {
        let Some(pt) = world.resource_mut::<PhysicsTime>() else { return };
        let elapsed = now.wrapping_sub(pt.last_tick);
        pt.last_tick = now;
        pt.accumulator += elapsed;
        let steps = pt.accumulator / PHYSICS_DT;
        pt.accumulator %= PHYSICS_DT;
        (steps,)
    };
    for _ in 0..steps.min(8) {
        three_body_step(world);
    }
}

fn three_body_step(world: &mut World) {
    const EQUILIBRIUM: Fixed = Fixed::from_int(30);
    let mut buf = Vec::new();
    world.query::<PhysicsBody>().and::<Velocity>().collect_into(&mut buf);
    let entities = buf;
    let mut positions = [(Fixed::ZERO, Fixed::ZERO); 3];
    for i in 0..3 {
        if let Some(body) = world.get::<PhysicsBody>(entities[i]) {
            positions[i] = (body.x, body.y);
        }
    }
    let mut ax = [Fixed::ZERO; 3];
    let mut ay = [Fixed::ZERO; 3];
    for i in 0..3 {
        for j in (i + 1)..3 {
            let dx = positions[j].0 - positions[i].0;
            let dy = positions[j].1 - positions[i].1;
            let dx_int = dx.to_int();
            let dy_int = dy.to_int();
            let dist = Fixed::from_int(isqrt((dx_int * dx_int + dy_int * dy_int) as u32) as i32);
            if dist == Fixed::ZERO { continue; }
            let force = Fixed::from_int(120) * (dist - EQUILIBRIUM) / dist;
            let fx = force * dx / (dist * dist);
            let fy = force * dy / (dist * dist);
            ax[i] += fx; ay[i] += fy;
            ax[j] -= fx; ay[j] -= fy;
        }
    }
    let clamp_max = Fixed::from_raw(1200);
    let clamp_min = Fixed::from_raw(-1200);
    for i in 0..3 {
        let e = entities[i];
        if let Some(vel) = world.get_mut::<Velocity>(e) {
            vel.vx += ax[i]; vel.vy += ay[i];
            if vel.vx.raw() > clamp_max.raw() { vel.vx = clamp_max; }
            if vel.vx.raw() < clamp_min.raw() { vel.vx = clamp_min; }
            if vel.vy.raw() > clamp_max.raw() { vel.vy = clamp_max; }
            if vel.vy.raw() < clamp_min.raw() { vel.vy = clamp_min; }
        }
        let (vx, vy) = world.get::<Velocity>(e).map(|v| (v.vx, v.vy)).unwrap_or((Fixed::ZERO, Fixed::ZERO));
        if let Some(body) = world.get_mut::<PhysicsBody>(e) {
            body.x += vx; body.y += vy;
            let min = Fixed::from_int(8);
            let max_x = Fixed::from_int(W as i32 - 8);
            let max_y = Fixed::from_int(H as i32 - 8);
            if body.x < min { body.x = min; }
            if body.x > max_x { body.x = max_x; }
            if body.y < min { body.y = min; }
            if body.y > max_y { body.y = max_y; }
        }
        if let Some(body) = world.get::<PhysicsBody>(e) {
            let bx = body.x; let by = body.y;
            if let Some(vel) = world.get_mut::<Velocity>(e) {
                if bx <= Fixed::from_int(8) || bx >= Fixed::from_int(W as i32 - 8) { vel.vx = Fixed::ZERO - vel.vx; }
                if by <= Fixed::from_int(8) || by >= Fixed::from_int(H as i32 - 8) { vel.vy = Fixed::ZERO - vel.vy; }
            }
        }
    }
}

fn kick_system(world: &mut World) {
    let fc = world.resource::<crate::FrameCounter>().map(|f| f.0).unwrap_or(0);
    let mut buf = Vec::new();
    world.query::<Velocity>().collect_into(&mut buf);
    let entities = buf;
    if fc % 40 == 0 && !entities.is_empty() {
        let kick_idx = (fc / 40) as usize % entities.len();
        let kick_dir = (fc / 120) as i32;
        let e = entities[kick_idx];
        if let Some(vel) = world.get_mut::<Velocity>(e) {
            vel.vx += Fixed::from_raw(((kick_dir * 7) % 13 - 6) * 160);
            vel.vy += Fixed::from_raw(((kick_dir * 11) % 13 - 6) * 160);
        }
    }
}

fn sync_layout_system(world: &mut World) {
    let iw = IMG_THUMBS_UP_WIDTH as i32;
    let ih = IMG_THUMBS_UP_HEIGHT as i32;
    let mut buf = Vec::new();
    world.query::<PhysicsBody>().collect_into(&mut buf);
    for e in buf {
        let (bx, by) = world.get::<PhysicsBody>(e)
            .map(|b| (b.x.to_int() - iw / 2, b.y.to_int() - ih / 2))
            .unwrap_or((0, 0));
        mirui::widget::set_position(world, e, bx, by);
    }
}

pub fn setup(app: &mut App<impl mirui::backend::Backend>) {
    app.add_system(physics_tick_system);
    app.add_system(kick_system);
    app.add_system(sync_layout_system);

    let world = &mut app.world;
    world.insert_resource(PhysicsTime { last_tick: systimer_now(), accumulator: 0 });

    let root = WidgetBuilder::new(world)
        .bg_color(Color::rgb(30, 30, 46))
        .layout(LayoutStyle { direction: FlexDirection::Column, width: Dimension::px(W as i32), height: Dimension::px(H as i32), ..Default::default() })
        .id();

    mirui_macros::ui! {
        :(
            parent: root
            world: world
        :)

        content (direction: FlexDirection::Column, grow: 1.0) {
            header (
                bg_color: Color::rgb(88, 166, 255),
                height: 20,
                text: "mirui",
                border_radius: 3
            ) {}
            row (direction: FlexDirection::Row, grow: 1.0) {
                left (bg_color: Color::rgb(63, 185, 80), grow: 1.0) {}
                right (bg_color: Color::rgb(248, 81, 73), grow: 1.0) {}
            }
            footer (bg_color: Color::rgb(210, 168, 255), height: 20, text: "3-body") {}
        }
    };

    let iw = IMG_THUMBS_UP_WIDTH;
    let ih = IMG_THUMBS_UP_HEIGHT;
    let cx = Fixed::from_int(W as i32 / 2);
    let cy = Fixed::from_int(H as i32 / 2);
    let r = Fixed::from_int(30);
    let r78 = r * 7 / 8;
    let init_pos = [
        (cx, cy - r, Fixed::from_raw(350), Fixed::ZERO),
        (cx - r78, cy + r / 2, Fixed::from_raw(-175), Fixed::from_raw(300)),
        (cx + r78, cy + r / 2, Fixed::from_raw(-175), Fixed::from_raw(-300)),
    ];

    mirui_macros::ui! {
        :(
            parent: root
            world: world
        :)

        walk init_pos.iter() with pos {
            body (
                position: Position::Absolute,
                left: pos.0.to_int() - iw as i32 / 2,
                top: pos.1.to_int() - ih as i32 / 2,
                width: iw,
                height: ih,
                image: Image::new(Vec::from(IMG_THUMBS_UP), iw, ih)
            ) [
                PhysicsBody { x: pos.0, y: pos.1 },
                Velocity { vx: pos.2, vy: pos.3 },
            ] {}
        }
    };

    app.set_root(root);
}
