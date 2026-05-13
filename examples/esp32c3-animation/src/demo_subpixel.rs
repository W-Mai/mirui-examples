use alloc::vec::Vec;
use mirui::app::App;
use mirui::ecs::World;
use mirui::layout::*;
use mirui::types::{Color, Dimension, Fixed};
use mirui::widget::builder::WidgetBuilder;

use crate::board::{H, W};

struct BarState { y: Fixed, speed: Fixed, snap: bool, x: Fixed }

fn bar_move_system(world: &mut World) {
    let mut buf = Vec::new();
    world.query::<BarState>().collect_into(&mut buf);
    for e in buf {
        let (new_y, changed) = {
            let Some(bar) = world.get_mut::<BarState>(e) else { continue };
            let old_display = if bar.snap { bar.y.floor() } else { bar.y };
            bar.y += bar.speed;
            if bar.y > Fixed::from_int(110) {
                bar.y = Fixed::from_int(20);
            }
            let new_display = if bar.snap { bar.y.floor() } else { bar.y };
            (new_display, new_display != old_display)
        };
        if changed {
            let bx = world.get::<BarState>(e).unwrap().x;
            mirui::widget::set_position(world, e, bx, new_y);
        }
    }
}

pub fn setup(app: &mut App<impl mirui::surface::FramebufferAccess>) {
    app.add_system(bar_move_system);

    let world = &mut app.world;

    let root = WidgetBuilder::new(world)
        .bg_color(Color::rgb(20, 20, 30))
        .layout(LayoutStyle { direction: FlexDirection::Column, width: Dimension::px(W as i32), height: Dimension::px(H as i32), ..Default::default() })
        .id();

    // Bar 1: integer snap (staircase)
    let bar1 = WidgetBuilder::new(world)
        .bg_color(Color::rgb(255, 100, 100))
        .layout(LayoutStyle {
            position: Position::Absolute,
            left: Dimension::px(10),
            top: Dimension::px(20),
            width: Dimension::px(50),
            height: Dimension::px(8),
            ..Default::default()
        })
        .id();
    world.insert(bar1, BarState { y: Fixed::from_int(20), speed: Fixed::from_raw(9), snap: true, x: Fixed::from_int(10) });

    // Bar 2: subpixel smooth
    let bar2 = WidgetBuilder::new(world)
        .bg_color(Color::rgb(100, 200, 255))
        .layout(LayoutStyle {
            position: Position::Absolute,
            left: Dimension::px(68),
            top: Dimension::px(20),
            width: Dimension::px(50),
            height: Dimension::px(8),
            ..Default::default()
        })
        .id();
    world.insert(bar2, BarState { y: Fixed::from_int(20), speed: Fixed::from_raw(9), snap: false, x: Fixed::from_int(68) });

    {
        use mirui::widget::{Children, Parent};
        world.insert(bar1, Parent(root));
        world.insert(bar2, Parent(root));
        if let Some(children) = world.get_mut::<Children>(root) {
            children.0.push(bar1);
            children.0.push(bar2);
        }
    }

    app.set_root(root);
}
