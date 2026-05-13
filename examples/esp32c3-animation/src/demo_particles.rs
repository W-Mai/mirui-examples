use alloc::vec::Vec;
use mirui::app::App;
use mirui::ecs::World;
use mirui::layout::*;
use mirui::types::{Color, Dimension, Fixed};
use mirui::widget::builder::WidgetBuilder;

use crate::board::{systimer_now, H, W};

/// Animated particles + pulsing rings + bouncing gradient bars
/// Demonstrates subpixel smoothness with multiple overlapping animations

struct Particle {
    x: Fixed,
    y: Fixed,
    vx: Fixed,
    vy: Fixed,
    phase: Fixed,
}

struct PulseRing {
    radius: Fixed,
    grow_speed: Fixed,
    max_radius: Fixed,
}

struct BouncingBar {
    pos: Fixed,
    speed: Fixed,
    vertical: bool,
}

fn particle_system(world: &mut World) {
    let mut buf = Vec::new();
    world.query::<Particle>().collect_into(&mut buf);
    for e in buf {
        let (new_x, new_y) = {
            let Some(p) = world.get_mut::<Particle>(e) else { continue };
            p.x += p.vx;
            p.y += p.vy;
            p.phase += Fixed::from_raw(5);

            // Bounce off walls
            if p.x < Fixed::from_int(2) || p.x > Fixed::from_int(W as i32 - 6) {
                p.vx = Fixed::ZERO - p.vx;
                p.x = p.x.max(Fixed::from_int(2)).min(Fixed::from_int(W as i32 - 6));
            }
            if p.y < Fixed::from_int(2) || p.y > Fixed::from_int(H as i32 - 6) {
                p.vy = Fixed::ZERO - p.vy;
                p.y = p.y.max(Fixed::from_int(2)).min(Fixed::from_int(H as i32 - 6));
            }
            (p.x, p.y)
        };
        mirui::widget::set_position(world, e, new_x, new_y);
    }
}

fn pulse_ring_system(world: &mut World) {
    let mut buf = Vec::new();
    world.query::<PulseRing>().collect_into(&mut buf);
    for e in buf {
        let new_radius = {
            let Some(ring) = world.get_mut::<PulseRing>(e) else { continue };
            ring.radius += ring.grow_speed;
            if ring.radius > ring.max_radius {
                ring.radius = Fixed::from_int(2);
            }
            ring.radius
        };
        if let Some(style) = world.get_mut::<mirui::widget::Style>(e) {
            let center_x = Fixed::from_int(W as i32 / 2);
            let center_y = Fixed::from_int(H as i32 / 2);
            style.layout.left = Dimension::Px(center_x - new_radius);
            style.layout.top = Dimension::Px(center_y - new_radius);
            style.layout.width = Dimension::Px(new_radius * 2);
            style.layout.height = Dimension::Px(new_radius * 2);
            style.border_radius = Fixed::ZERO;
        }
        world.insert(e, mirui::widget::dirty::Dirty);
    }
}

fn bar_system(world: &mut World) {
    let mut buf = Vec::new();
    world.query::<BouncingBar>().collect_into(&mut buf);
    for e in buf {
        let (new_x, new_y) = {
            let Some(bar) = world.get_mut::<BouncingBar>(e) else { continue };
            bar.pos += bar.speed;
            let max = if bar.vertical {
                Fixed::from_int(H as i32 - 20)
            } else {
                Fixed::from_int(W as i32 - 30)
            };
            if bar.pos < Fixed::from_int(4) || bar.pos > max {
                bar.speed = Fixed::ZERO - bar.speed;
                bar.pos = bar.pos.max(Fixed::from_int(4)).min(max);
            }
            if bar.vertical {
                (Fixed::from_int(4), bar.pos)
            } else {
                (bar.pos, Fixed::from_int(4))
            }
        };
        mirui::widget::set_position(world, e, new_x, new_y);
    }
}

pub fn setup(app: &mut App<impl mirui::surface::FramebufferAccess>) {
    app.add_system(particle_system);
    app.add_system(pulse_ring_system);
    app.add_system(bar_system);

    let world = &mut app.world;

    let root = WidgetBuilder::new(world)
        .bg_color(Color::rgb(10, 10, 20))
        .layout(LayoutStyle {
            width: Dimension::px(W as i32),
            height: Dimension::px(H as i32),
            ..Default::default()
        })
        .id();

    // Pulsing rings (3 concentric, different speeds)
    let ring_colors = [
        Color::rgba(80, 200, 255, 60),
        Color::rgba(255, 100, 200, 40),
        Color::rgba(100, 255, 150, 50),
    ];
    let ring_speeds = [
        Fixed::from_raw(12),
        Fixed::from_raw(8),
        Fixed::from_raw(15),
    ];
    let ring_max = [
        Fixed::from_int(20),
        Fixed::from_int(16),
        Fixed::from_int(22),
    ];

    for i in 0..3 {
        let ring = WidgetBuilder::new(world)
            .bg_color(ring_colors[i])
            .border(ring_colors[i], Fixed::from_int(2))
            .border_radius(Fixed::from_int(10))
            .layout(LayoutStyle {
                position: Position::Absolute,
                left: Dimension::px(W as i32 / 2 - 10),
                top: Dimension::px(H as i32 / 2 - 10),
                width: Dimension::px(20),
                height: Dimension::px(20),
                ..Default::default()
            })
            .id();
        world.insert(ring, PulseRing {
            radius: Fixed::from_int(5 + i as i32 * 8),
            grow_speed: ring_speeds[i],
            max_radius: ring_max[i],
        });
        world.insert(ring, mirui::widget::Parent(root));
        if let Some(ch) = world.get_mut::<mirui::widget::Children>(root) {
            ch.0.push(ring);
        }
    }

    // Bouncing bars (2 horizontal, 1 vertical)
    let bar_configs: [(Color, Fixed, Fixed, bool, i32, i32); 3] = [
        (Color::rgba(255, 200, 50, 180), Fixed::from_raw(45), Fixed::from_int(10), false, 30, 6),
        (Color::rgba(50, 255, 200, 160), Fixed::from_raw(33), Fixed::from_int(80), false, 25, 5),
        (Color::rgba(200, 50, 255, 140), Fixed::from_raw(55), Fixed::from_int(20), true, 5, 40),
    ];

    for (color, speed, start, vertical, bw, bh) in bar_configs {
        let bar = WidgetBuilder::new(world)
            .bg_color(color)
            .border_radius(Fixed::ZERO)
            .layout(LayoutStyle {
                position: Position::Absolute,
                left: Dimension::px(4),
                top: Dimension::px(4),
                width: Dimension::px(bw),
                height: Dimension::px(bh),
                ..Default::default()
            })
            .id();
        world.insert(bar, BouncingBar { pos: start, speed, vertical });
        world.insert(bar, mirui::widget::Parent(root));
        if let Some(ch) = world.get_mut::<mirui::widget::Children>(root) {
            ch.0.push(bar);
        }
    }

    // Floating particles (6 small squares)
    let mut rng_state: u32 = systimer_now();
    let mut rng = || -> i32 {
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 17;
        rng_state ^= rng_state << 5;
        (rng_state % 256) as i32
    };

    let particle_colors = [
        Color::rgb(255, 80, 80),
        Color::rgb(80, 255, 80),
        Color::rgb(80, 80, 255),
        Color::rgb(255, 255, 80),
        Color::rgb(255, 80, 255),
        Color::rgb(80, 255, 255),
    ];

    for i in 0..6 {
        let px = Fixed::from_raw(rng() % (100 * 256));
        let py = Fixed::from_raw(rng() % (100 * 256));
        let vx = Fixed::from_raw(rng() % 200 - 100);
        let vy = Fixed::from_raw(rng() % 200 - 100);

        let particle = WidgetBuilder::new(world)
            .bg_color(particle_colors[i])
            .border_radius(Fixed::ZERO)
            .layout(LayoutStyle {
                position: Position::Absolute,
                left: Dimension::Px(px),
                top: Dimension::Px(py),
                width: Dimension::px(4),
                height: Dimension::px(4),
                ..Default::default()
            })
            .id();
        world.insert(particle, Particle { x: px, y: py, vx, vy, phase: Fixed::ZERO });
        world.insert(particle, mirui::widget::Parent(root));
        if let Some(ch) = world.get_mut::<mirui::widget::Children>(root) {
            ch.0.push(particle);
        }
    }

    app.set_root(root);
}
