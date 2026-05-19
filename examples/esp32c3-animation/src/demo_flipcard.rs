use alloc::vec::Vec;
use mirui::app::App;
use mirui::components::assets::*;
use mirui::components::image::Image;
use mirui::components::transform_3d::WidgetTransform3D;
use mirui::ecs::World;
use mirui::layout::*;
use mirui::types::{Color, Dimension, Fixed, Transform3D};
use mirui::widget::builder::WidgetBuilder;
use mirui::widget::dirty::Dirty;
use mirui::widget::{Children, Parent};

struct Spinner {
    angle: Fixed,
    speed: Fixed,
    bounce_phase: Fixed,
}

#[mirui::system(order = ANIMATION)]
fn spin_system(world: &mut World) {
    let mut entities = Vec::new();
    world.query::<Spinner>().collect_into(&mut entities);
    for e in entities {
        let (angle, bounce) = if let Some(s) = world.get_mut::<Spinner>(e) {
            s.angle += s.speed;
            if s.angle >= Fixed::from_int(360) {
                s.angle -= Fixed::from_int(360);
            }
            // Bounce locks to the rotation (1x for now).
            s.bounce_phase += s.speed;
            if s.bounce_phase >= Fixed::from_int(360) {
                s.bounce_phase -= Fixed::from_int(360);
            }
            (s.angle, s.bounce_phase)
        } else {
            continue;
        };

        // h(t) = 1 - (2t - 1)² — parabolic hop: apex hovers, impact is fastest.
        // One bounce spans bounce_phase ∈ [0°, 180°), so t = (bounce % 180) / 180.
        let t_num = bounce.to_int() % 180;
        let t = Fixed::from_int(t_num) / Fixed::from_int(180);
        let two_t_minus_1 = t * Fixed::from_int(2) - Fixed::ONE;
        let h = Fixed::ONE - two_t_minus_1 * two_t_minus_1;

        let bounce_y = Fixed::ZERO - h * Fixed::from_int(45);
        // Ground-contact squash: (1 - h) grows from 0 at apex to 1 at
        // the floor, cubed so only the last sliver of the drop feels
        // the compression. Keeps the apex perfectly round.
        let one_minus_h = Fixed::ONE - h;
        let cubed = one_minus_h * one_minus_h * one_minus_h;
        let squash = Fixed::ONE - cubed / Fixed::from_int(4);
        let stretch = Fixed::ONE;
        let rot = Transform3D::rotate_y_perspective(angle, Fixed::from_int(150));
        let scale = Transform3D::scale(squash, stretch);
        let translate = Transform3D::translate(Fixed::ZERO, bounce_y);
        world.insert(
            e,
            WidgetTransform3D(translate.compose(&rot).compose(&scale)),
        );
        world.insert(e, Dirty);
    }
}

pub fn setup<B: mirui::surface::FramebufferAccess>(app: &mut App<B>) {
    let (logical_w, logical_h) = {
        let info = app.backend.display_info();
        (info.width as i32, info.height as i32)
    };

    app.add_system(spin_system::system());

    let world = &mut app.world;

    let root = WidgetBuilder::new(world)
        .bg_color(Color::rgb(30, 30, 46))
        .layout(LayoutStyle {
            direction: FlexDirection::Column,
            width: Dimension::px(logical_w),
            height: Dimension::px(logical_h),
            ..Default::default()
        })
        .id();

    let side = 32;
    let img = WidgetBuilder::new(world)
        .layout(LayoutStyle {
            position: Position::Absolute,
            left: Dimension::px((logical_w - side) / 2),
            top: Dimension::px(logical_h - side - 8),
            width: Dimension::px(side),
            height: Dimension::px(side),
            ..Default::default()
        })
        .id();
    world.insert(img, Image::new(&IMG_THUMBS_UP));
    world.insert(
        img,
        Spinner {
            angle: Fixed::ZERO,
            speed: Fixed::from_int(3),
            bounce_phase: Fixed::ZERO,
        },
    );
    world.insert(img, Parent(root));
    if let Some(children) = world.get_mut::<Children>(root) {
        children.0.push(img);
    }

    app.set_root(root);
}
