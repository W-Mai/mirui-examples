use mirui::anim::{Spring, ease};
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
use mirui::widget::theme::{Theme, ThemedColor};

use alloc::vec::Vec;

mirui_macros::animate!(AnimateThumbX, |world, entity, value| {
    mirui::widget::set_position(world, entity, value, Fixed::from_int(2));
});

// Spring-driven 0..1 → off_color..on_color crossfade so toggling reads
// as a transition, not a frame-snapped color flip.
mirui_macros::animate!(AnimateSwitchBgT, |world, entity, value| {
    let Some(sw) = world.get::<Switch>(entity) else {
        return;
    };
    let theme = world
        .resource::<Theme>()
        .cloned()
        .unwrap_or_else(Theme::dark);
    let off = sw.off_color.resolve(&theme);
    let on = sw.on_color.resolve(&theme);
    let color = Color::lerp(off, on, value);
    if let Some(style) = world.get_mut::<mirui::widget::Style>(entity) {
        style.bg_color = Some(ThemedColor::Raw(color));
    }
    world.insert(entity, Dirty);
});

struct SwitchBgT(Fixed);

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
    let (clamped_ratio, fill_color) = {
        let Some(slider) = world.get_mut::<Slider>(entity) else {
            return false;
        };
        slider.set_ratio(ratio);
        (slider.ratio(), slider.fill_color)
    };
    let fill_w = clamped_ratio * track_w;
    if let Some(children) = world.get::<mirui::widget::Children>(entity) {
        let cc: Vec<Entity> = children.0.clone();
        if cc.len() >= 2 {
            let fill_mask = cc[0];
            let thumb_entity = cc[1];
            if let Some(style) = world.get_mut::<mirui::widget::Style>(fill_mask) {
                style.layout.width = Dimension::Px(fill_w);
            }
            if let Some(mask_children) = world.get::<mirui::widget::Children>(fill_mask) {
                if let Some(&fill_inner) = mask_children.0.first() {
                    if let Some(style) = world.get_mut::<mirui::widget::Style>(fill_inner) {
                        style.bg_color = Some(fill_color);
                    }
                    world.insert(fill_inner, Dirty);
                }
            }
            world.insert(fill_mask, Dirty);
            let thumb_w = Fixed::from_int(10);
            let thumb_x = clamped_ratio * (track_w - thumb_w);
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
    let target_t = if is_on { Fixed::ONE } else { Fixed::ZERO };
    let current_t = world
        .get::<SwitchBgT>(entity)
        .map(|t| t.0)
        .unwrap_or_else(|| if is_on { Fixed::ZERO } else { Fixed::ONE });
    world.insert(entity, SwitchBgT(target_t));
    world.insert(
        entity,
        AnimateSwitchBgT(Spring::new(current_t, target_t, 250, Fixed::ZERO).into()),
    );
    world.insert(entity, Dirty);

    if let Some(children) = world.get::<mirui::widget::Children>(entity) {
        if let Some(&thumb) = children.0.first() {
            // track 34 wide, thumb 14 wide, 2 px inset on each side ⇒
            // off=2, on=34-14-2=18 keeps both ends symmetric.
            let target_x = if is_on {
                Fixed::from_int(18)
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
                AnimateThumbX(Spring::new(current_x, target_x, 200, Fixed::ZERO).into()),
            );
        }
    }
    true
}

pub fn setup<B: mirui::surface::FramebufferAccess>(app: &mut App<B>) {
    use mirui::ecs::{System, run_order};
    app.add_system(System::new(
        "animate_thumb_x",
        run_order::ANIMATION,
        AnimateThumbX::system(),
    ));
    app.add_system(System::new(
        "animate_switch_bg_t",
        run_order::ANIMATION,
        AnimateSwitchBgT::system(),
    ));
    app.add_system(sim_timeline_system::system());

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
            fill_mask (
                width: 54,
                height: 10,
                clip_children: true
            ) {
                fill_inner (
                    bg_color: Color::rgb(88, 166, 255),
                    position: Position::Absolute,
                    left: 0,
                    top: 0,
                    width: 108,
                    height: 10,
                    border_radius: 5
                ) {}
            }
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

    // PointerDown must land inside the slider's hit rect (x=10..118).
    // Drag from a point inside, then over-shoot the destination so
    // set_ratio clamps to the exact endpoint (DragMove past the rect
    // is fine — gesture targets latch on PointerDown).
    app.world.insert_resource(
        SimTimeline::new(alloc::vec![
            SimAction::drag(
                Point::new(10, 15),
                Point::new(120, 15),
                500,
                ease::ease_in_out_cubic,
            ),
            SimAction::wait(1000),
            SimAction::tap(Point::new(27, 50)),
            SimAction::wait(1000),
            SimAction::drag(
                Point::new(117, 15),
                Point::new(8, 15),
                300,
                ease::ease_in_out_cubic,
            ),
            SimAction::wait(1000),
            SimAction::tap(Point::new(27, 50)),
            SimAction::wait(1000),
        ])
        .looping(true),
    );

    app.set_root(root);
}
