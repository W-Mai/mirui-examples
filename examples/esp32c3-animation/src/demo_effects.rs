use mirui::anim::{PlayMode, Tween, ease};
use mirui::components::{BackgroundBlur, MirrorOf, Text};
use mirui::prelude::*;

use crate::board::{H, W};

mirui_macros::animate!(GlassX, |world, entity, value| {
    mirui::widget::set_position(world, entity, value, Fixed::from_int(50));
});

mirui_macros::animate!(GaussRadius, |world, entity, value| {
    if let Some(blur) = world.get_mut::<BackgroundBlur>(entity) {
        blur.radius = value;
    }
});

const TILE_COLORS: [Color; 4] = [
    Color::rgb(220, 60, 60),
    Color::rgb(220, 160, 40),
    Color::rgb(60, 200, 80),
    Color::rgb(40, 140, 220),
];

fn tile_color(row: i32, col: i32) -> Color {
    TILE_COLORS[((row + col) as usize) % TILE_COLORS.len()]
}

pub fn setup(app: &mut App<impl mirui::surface::FramebufferAccess>) {
    app.add_system(mirui::ecs::System::new(
        "glass_x",
        mirui::ecs::run_order::ANIMATION,
        GlassX::system(),
    ));
    app.add_system(mirui::ecs::System::new(
        "gauss_radius",
        mirui::ecs::run_order::ANIMATION,
        GaussRadius::system(),
    ));

    let root = WidgetBuilder::new(&mut app.world)
        .bg_color(Color::rgb(20, 22, 28))
        .layout(LayoutStyle {
            width: Dimension::px(W as i32),
            height: Dimension::px(H as i32),
            ..Default::default()
        })
        .id();

    ui! {
        :( parent: root world: &mut app.world :)

        View (
            position: Position::Absolute,
            left: 0,
            top: 0,
            width: W as i32,
            height: H as i32
        ) {
            walk 0..3i32 with row {
                walk 0..4i32 with col {
                    View (
                        bg_color: tile_color(row, col),
                        position: Position::Absolute,
                        left: col * 32,
                        top: row * 32,
                        width: 32,
                        height: 32
                    ) {}
                }
            }
        }
    };

    let m_source = ui! {
        :( parent: root world: &mut app.world :)

        View (
            bg_color: Color::rgb(80, 160, 255),
            position: Position::Absolute,
            left: 8,
            top: 8,
            width: 40,
            height: 14
        ) {}
    };

    ui! {
        :( parent: root world: &mut app.world :)

        View (
            position: Position::Absolute,
            left: 8,
            top: 24,
            width: 40,
            height: 14
        ) [
            MirrorOf::new(m_source).with_fade(180),
        ] {}
    };

    ui! {
        :( parent: root world: &mut app.world :)

        View (
            text: "BlurMeBlurMe",
            position: Position::Absolute,
            left: 8,
            top: 58,
            width: W as i32 - 16,
            height: 14
        ) [
            Text(b"BlurMeBlurMe".to_vec()),
        ] {}
    };

    ui! {
        :( parent: root world: &mut app.world :)

        View (
            bg_color: Color::rgba(255, 255, 255, 50),
            border_radius: Fixed::from_int(6),
            position: Position::Absolute,
            left: 30,
            top: 50,
            width: 60,
            height: 30
        ) [
            BackgroundBlur::new(2),
            GlassX(
                Tween::new(
                    Fixed::from_int(8),
                    Fixed::from_int(W as i32 - 60 - 8),
                    3000,
                    ease::ease_in_out_cubic,
                    PlayMode::PingPong,
                )
                .into(),
            ),
            GaussRadius(
                Tween::new(
                    Fixed::from_int(0),
                    Fixed::from_int(3),
                    3000,
                    ease::ease_in_out_cubic,
                    PlayMode::PingPong,
                ).into(),
            )
        ] {}
    };

    app.with_offscreen_pool_budget(8 * 1024);
    app.set_root(root);
}
