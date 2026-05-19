use mirui::app::App;
use mirui::components::assets::IMG_THUMBS_UP;
use mirui::components::image::Image;
use mirui::event::scroll::{ScrollAxis, ScrollConfig, ScrollOffset};
use mirui::components::transform_3d::WidgetTransform3D;
use mirui::ecs::World;
use mirui::layout::*;
use mirui::types::{Color, Dimension, Fixed, Transform3D};
use mirui::widget::builder::WidgetBuilder;
use mirui::widget::dirty::Dirty;

const CARD_W: i32 = 40;
const CARD_H: i32 = 56;
const CARD_GAP: i32 = 10;
const PERSPECTIVE: i32 = 160;
const CARD_COUNT: i32 = 5;
const STRIDE: i32 = CARD_W + CARD_GAP;

struct CarouselCard {
    index: usize,
}

struct Carousel;

struct SwayPhase(Fixed);
struct ScreenSize {
    w: i32,
    h: i32,
}

fn sway_system(world: &mut World) {
    let phase = {
        let Some(p) = world.resource_mut::<SwayPhase>() else {
            return;
        };
        p.0 += Fixed::ONE;
        if p.0 >= Fixed::from_int(360) {
            p.0 -= Fixed::from_int(360);
        }
        p.0
    };

    let mut carousels = alloc::vec::Vec::new();
    world.query::<Carousel>().collect_into(&mut carousels);
    let Some(&carousel) = carousels.first() else {
        return;
    };

    let logical_w = world.resource::<ScreenSize>().map(|s| s.w).unwrap_or(128);
    let centred =
        Fixed::from_int(STRIDE * (CARD_COUNT / 2)) - Fixed::from_int((logical_w - CARD_W) / 2);
    let amplitude = Fixed::from_int(STRIDE * (CARD_COUNT - 1) / 2);
    let new_offset = centred + Fixed::sin_deg(phase) * amplitude;

    if let Some(scroll) = world.get_mut::<ScrollOffset>(carousel) {
        scroll.x = new_offset;
    }
}

fn layout_system(world: &mut World) {
    let mut carousels = alloc::vec::Vec::new();
    world.query::<Carousel>().collect_into(&mut carousels);
    let offset = match carousels.first().and_then(|&e| world.get::<ScrollOffset>(e)) {
        Some(s) => s.x,
        None => return,
    };

    let (logical_w, logical_h) = world
        .resource::<ScreenSize>()
        .map(|s| (s.w, s.h))
        .unwrap_or((128, 128));

    let mut cards = alloc::vec::Vec::new();
    world.query::<CarouselCard>().collect_into(&mut cards);
    if cards.is_empty() {
        return;
    }
    let slot_stride = Fixed::from_int(STRIDE);
    let centre_offset = Fixed::from_int((logical_w - CARD_W) / 2);

    for e in cards {
        let idx = match world.get::<CarouselCard>(e) {
            Some(c) => c.index as i32,
            None => continue,
        };
        let tx = Fixed::from_int(idx * STRIDE);
        let ty = Fixed::from_int((logical_h - CARD_H) / 2);
        mirui::widget::set_position(world, e, tx, ty);

        let relative = Fixed::from_int(idx) - (offset + centre_offset) / slot_stride;
        let tilt_y = Fixed::ZERO - relative * Fixed::from_int(40);
        let phase_x = world
            .resource::<SwayPhase>()
            .map(|p| p.0)
            .unwrap_or(Fixed::ZERO);
        let tilt_x =
            Fixed::sin_deg(phase_x + relative * Fixed::from_int(60)) * Fixed::from_int(35);
        let distance = Fixed::from_int(PERSPECTIVE);
        let ty = Transform3D::rotate_y_perspective(tilt_y, distance);
        let tx3d = Transform3D::rotate_x_perspective(tilt_x, distance);
        world.insert(e, WidgetTransform3D(ty.compose(&tx3d)));
        world.insert(e, Dirty);
    }
}

pub fn setup<B: mirui::surface::FramebufferAccess>(app: &mut App<B>) {
    let (logical_w, logical_h) = {
        let info = app.backend.display_info();
        (info.width as i32, info.height as i32)
    };

    use mirui::ecs::{System, run_order};
    app.add_system(System::new("sway", run_order::ANIMATION, sway_system));
    app.add_system(System::new("layout", run_order::NORMAL, layout_system));
    app.world.insert_resource(SwayPhase(Fixed::ZERO));
    app.world.insert_resource(ScreenSize {
        w: logical_w,
        h: logical_h,
    });

    let world = &mut app.world;

    let root = WidgetBuilder::new(world)
        .bg_color(Color::rgb(18, 20, 28))
        .layout(LayoutStyle {
            direction: FlexDirection::Column,
            width: Dimension::px(logical_w),
            height: Dimension::px(logical_h),
            ..Default::default()
        })
        .id();

    let card_colors = [
        Color::rgb(255, 107, 107),
        Color::rgb(255, 206, 84),
        Color::rgb(136, 216, 176),
        Color::rgb(118, 209, 244),
        Color::rgb(178, 148, 255),
    ];
    let card_colors_ref = &card_colors;

    mirui_macros::ui! {
        :(
            parent: root
            world: world
        :)

        carousel (
            position: Position::Absolute,
            left: 0,
            top: 0,
            width: logical_w,
            height: logical_h
        ) [
            Carousel,
            ScrollOffset {
                x: Fixed::from_int(
                    STRIDE * (CARD_COUNT / 2) - (logical_w - CARD_W) / 2,
                ),
                y: Fixed::ZERO,
            },
            ScrollConfig {
                direction: ScrollAxis::Horizontal,
                elastic: false,
                content_width: Fixed::from_int(STRIDE * CARD_COUNT),
                content_height: Fixed::ZERO,
            },
        ] {
            walk card_colors_ref.iter().enumerate() with item {
                card (
                    position: Position::Absolute,
                    left: 0,
                    top: 0,
                    width: CARD_W,
                    height: CARD_H,
                    bg_color: *item.1,
                    border_radius: 6
                ) [
                    CarouselCard { index: item.0 },
                ] {
                    if item.0 % 2 == 1 {
                        thumb (
                            position: Position::Absolute,
                            left: 4,
                            top: 4,
                            width: CARD_W - 8,
                            height: CARD_H - 8,
                            image: Image::new(&IMG_THUMBS_UP)
                        ) {}
                    }
                }
            }
        }
    };

    app.set_root(root);
}
