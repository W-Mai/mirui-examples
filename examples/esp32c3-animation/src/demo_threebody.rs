use alloc::vec;
use alloc::vec::Vec;
use mirui::app::App;
use mirui::components::assets::*;
use mirui::components::image::Image;
use mirui::ecs::World;
use mirui::layout::*;
use mirui::types::{Color, Dimension, Fixed};
use mirui::widget::builder::WidgetBuilder;

use crate::board::systimer_now;

struct Velocity {
    vx: Fixed,
    vy: Fixed,
}
struct PhysicsBody {
    x: Fixed,
    y: Fixed,
}
struct PhysicsTime {
    last_tick: u32,
    accumulator: u32,
}

/// Logical-space bounds the bodies bounce in. Read by the physics
/// systems each tick; written once in `setup` from backend
/// `display_info().width/height` so HiDPI (scale != 1) just works.
struct WorldBounds {
    w: i32,
    h: i32,
}

/// Spring rest length for the N-body attractive-repulsive force.
/// Tuned per demo variant — small values (≈ 30) keep the classic
/// 128-logical three-body chaos; larger values suit wider viewports.
struct SpringLength(Fixed);

/// Scratch buffers re-used by `three_body_step` to avoid per-frame
/// heap allocation. Sized once in `setup` to the body count. Default
/// is used by `mem::take` during the in-world swap dance.
#[derive(Default)]
struct PhysicsScratch {
    entities: Vec<mirui::ecs::Entity>,
    positions: Vec<(Fixed, Fixed)>,
    ax: Vec<Fixed>,
    ay: Vec<Fixed>,
}

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
    let (bound_w, bound_h) = world
        .resource::<WorldBounds>()
        .map(|b| (b.w, b.h))
        .unwrap_or((128, 128));
    let equilibrium = world
        .resource::<SpringLength>()
        .map(|s| s.0)
        .unwrap_or(Fixed::from_int(30));

    // Swap the scratch buffers out by value (mem::take leaves empty
    // Vecs behind) so we can still hold &mut World for get/get_mut.
    // Restore at the end to keep capacity allocated across frames.
    let mut scratch = {
        let Some(s) = world.resource_mut::<PhysicsScratch>() else {
            return;
        };
        core::mem::take(s)
    };
    scratch.entities.clear();
    world
        .query::<PhysicsBody>()
        .and::<Velocity>()
        .collect_into(&mut scratch.entities);
    let n = scratch.entities.len();
    if n == 0 {
        if let Some(s) = world.resource_mut::<PhysicsScratch>() {
            *s = scratch;
        }
        return;
    }

    scratch.positions.clear();
    scratch.positions.resize(n, (Fixed::ZERO, Fixed::ZERO));
    for i in 0..n {
        if let Some(body) = world.get::<PhysicsBody>(scratch.entities[i]) {
            scratch.positions[i] = (body.x, body.y);
        }
    }

    scratch.ax.clear();
    scratch.ax.resize(n, Fixed::ZERO);
    scratch.ay.clear();
    scratch.ay.resize(n, Fixed::ZERO);
    for i in 0..n {
        for j in (i + 1)..n {
            let dx = scratch.positions[j].0 - scratch.positions[i].0;
            let dy = scratch.positions[j].1 - scratch.positions[i].1;
            let dx_int = dx.to_int();
            let dy_int = dy.to_int();
            let dist = Fixed::from_int(isqrt((dx_int * dx_int + dy_int * dy_int) as u32) as i32);
            if dist == Fixed::ZERO {
                continue;
            }
            let force = Fixed::from_int(120) * (dist - equilibrium) / dist;
            let fx = force * dx / (dist * dist);
            let fy = force * dy / (dist * dist);
            scratch.ax[i] += fx;
            scratch.ay[i] += fy;
            scratch.ax[j] -= fx;
            scratch.ay[j] -= fy;
        }
    }

    let v_max = Fixed::from_int(5);
    let v_min = Fixed::ZERO - v_max;
    let min = Fixed::from_int(8);
    let max_x = Fixed::from_int(bound_w - 8);
    let max_y = Fixed::from_int(bound_h - 8);
    for i in 0..n {
        let e = scratch.entities[i];
        if let Some(vel) = world.get_mut::<Velocity>(e) {
            vel.vx += scratch.ax[i];
            vel.vy += scratch.ay[i];
            if vel.vx > v_max {
                vel.vx = v_max;
            }
            if vel.vx < v_min {
                vel.vx = v_min;
            }
            if vel.vy > v_max {
                vel.vy = v_max;
            }
            if vel.vy < v_min {
                vel.vy = v_min;
            }
        }
        let (vx, vy) = world
            .get::<Velocity>(e)
            .map(|v| (v.vx, v.vy))
            .unwrap_or((Fixed::ZERO, Fixed::ZERO));
        if let Some(body) = world.get_mut::<PhysicsBody>(e) {
            body.x += vx;
            body.y += vy;
            if body.x < min {
                body.x = min;
            }
            if body.x > max_x {
                body.x = max_x;
            }
            if body.y < min {
                body.y = min;
            }
            if body.y > max_y {
                body.y = max_y;
            }
        }
        if let Some(body) = world.get::<PhysicsBody>(e) {
            let bx = body.x;
            let by = body.y;
            if let Some(vel) = world.get_mut::<Velocity>(e) {
                if bx <= min || bx >= max_x {
                    vel.vx = Fixed::ZERO - vel.vx;
                }
                if by <= min || by >= max_y {
                    vel.vy = Fixed::ZERO - vel.vy;
                }
            }
        }
    }

    if let Some(s) = world.resource_mut::<PhysicsScratch>() {
        *s = scratch;
    }
}

fn kick_system(world: &mut World) {
    let fc = world
        .resource::<crate::FrameCounter>()
        .map(|f| f.0)
        .unwrap_or(0);
    let mut buf = Vec::new();
    world.query::<Velocity>().collect_into(&mut buf);
    let entities = buf;
    if fc % 40 == 0 && !entities.is_empty() {
        let kick_idx = (fc / 40) as usize % entities.len();
        let kick_dir = (fc / 120) as i32;
        let e = entities[kick_idx];
        // Tiny perturbation: pseudo-random ±3-ish in Fixed units,
        // stepped via frame counter. Avoids literal raw fixed values.
        let kx = (kick_dir * 7).rem_euclid(13) - 6;
        let ky = (kick_dir * 11).rem_euclid(13) - 6;
        if let Some(vel) = world.get_mut::<Velocity>(e) {
            vel.vx += Fixed::from_int(kx) / Fixed::from_int(2);
            vel.vy += Fixed::from_int(ky) / Fixed::from_int(2);
        }
    }
}

fn sync_layout_system(world: &mut World) {
    let half_w = Fixed::from_int(IMG_THUMBS_UP.width as i32 / 2);
    let half_h = Fixed::from_int(IMG_THUMBS_UP.height as i32 / 2);
    let mut buf = Vec::new();
    world.query::<PhysicsBody>().collect_into(&mut buf);
    for e in buf {
        let (bx, by) = world.get::<PhysicsBody>(e)
            .map(|b| (b.x - half_w, b.y - half_h))
            .unwrap_or((Fixed::ZERO, Fixed::ZERO));
        mirui::widget::set_position(world, e, bx, by);
    }
}

/// Build an `n_bodies`-body demo. Reads logical screen dims from the
/// backend, so HiDPI (`scale != 1`) does not require any demo-side
/// change — bodies always spread across the full logical viewport.
/// `equilibrium` is the spring rest length for the inter-body force
/// (classic 128-logical three-body chaos runs well at 30).
pub fn setup<B: mirui::backend::FramebufferAccess>(
    app: &mut App<B>,
    n_bodies: usize,
    equilibrium: Fixed,
) {
    let (logical_w, logical_h) = {
        let info = app.backend.display_info();
        (info.width as i32, info.height as i32)
    };

    app.add_system(physics_tick_system);
    app.add_system(kick_system);
    app.add_system(sync_layout_system);

    let world = &mut app.world;
    world.insert_resource(PhysicsTime {
        last_tick: systimer_now(),
        accumulator: 0,
    });
    world.insert_resource(WorldBounds {
        w: logical_w,
        h: logical_h,
    });
    world.insert_resource(SpringLength(equilibrium));
    let n = n_bodies.max(1);
    world.insert_resource(PhysicsScratch {
        entities: Vec::with_capacity(n),
        positions: Vec::with_capacity(n),
        ax: Vec::with_capacity(n),
        ay: Vec::with_capacity(n),
    });

    let root = WidgetBuilder::new(world)
        .bg_color(Color::rgb(30, 30, 46))
        .layout(LayoutStyle {
            direction: FlexDirection::Column,
            width: Dimension::px(logical_w),
            height: Dimension::px(logical_h),
            ..Default::default()
        })
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
            footer (bg_color: Color::rgb(210, 168, 255), height: 20, text: "n-body") {}
        }
    };

    let iw = IMG_THUMBS_UP.width as i32;
    let ih = IMG_THUMBS_UP.height as i32;
    let cx = Fixed::from_int(logical_w / 2);
    let cy = Fixed::from_int(logical_h / 2);
    // Orbit radius ≈ 35% of the smaller logical dim; leaves slack for bounces.
    let r = Fixed::from_int(logical_w.min(logical_h) * 35 / 100);
    // Tangent speed in units/step; picked to look lively at 128 logical.
    let orbital = Fixed::from_int(2);

    let mut init_pos: Vec<(Fixed, Fixed, Fixed, Fixed)> = Vec::with_capacity(n_bodies.max(1));
    let n = n_bodies.max(1);
    for i in 0..n {
        let deg = Fixed::from_int(360) * Fixed::from_int(i as i32) / Fixed::from_int(n as i32);
        let c = Fixed::cos_deg(deg);
        let s = Fixed::sin_deg(deg);
        init_pos.push((cx + c * r, cy + s * r, Fixed::ZERO - s * orbital, c * orbital));
    }

    mirui_macros::ui! {
        :(
            parent: root
            world: world
        :)

        walk init_pos.iter() with pos {
            body (
                position: Position::Absolute,
                left: pos.0.to_int() - iw / 2,
                top: pos.1.to_int() - ih / 2,
                width: iw,
                height: ih,
                image: Image::new(&IMG_THUMBS_UP)
            ) [
                PhysicsBody { x: pos.0, y: pos.1 },
                Velocity { vx: pos.2, vy: pos.3 },
            ] {}
        }
    };

    app.set_root(root);
}
