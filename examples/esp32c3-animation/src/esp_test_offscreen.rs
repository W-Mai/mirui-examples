//! Framebuffer diagnostics for full and clipped offscreen rendering.
//!
//! App prepares text geometry and retained outputs before each comparison.
//! Fixtures deliberately manipulate ECS state without animation systems;
//! clipped draws exercise the low-level renderer against the completed frame.
//! Frame bytes and FNV-1a hashes are emitted through the supplied reporter.

use core::fmt::Arguments;
use mirui::app::App;
use mirui::ecs::{Entity, World};
use mirui::render::sw::SwRenderer;
use mirui::render::texture::ColorFormat;
use mirui::surface::framebuf::FramebufSurface;
use mirui::surface::{FramebufferAccess, Surface};
use mirui::types::{Dimension, Fixed, PhysicalRect, Rect};
use mirui::ui::dirty::Dirty;
use mirui::ui::layout::{AlignItems, FlexDirection, LayoutStyle, Padding};
use mirui::ui::theme::{ColorToken, ThemedColor};
use mirui::ui::widgets::{ProgressBar, Slider, Switch, Text};
use mirui::ui::{Children, OffscreenRender, Parent, Style, Widget, render_system};

const FB_W: u16 = 128;
const FB_H: u16 = 128;
const FB_BYTES: usize = FB_W as usize * FB_H as usize * 2;

type TestApp = App<FramebufSurface<fn(&[u8], PhysicalRect)>>;

fn ignore_flush(_: &[u8], _: PhysicalRect) {}

fn add_child(world: &mut World, parent: Entity, child: Entity) {
    world.insert(child, Parent(parent));
    if let Some(c) = world.get_mut::<Children>(parent) {
        c.0.push(child);
    } else {
        world.insert(parent, Children(alloc::vec![child]));
    }
}

fn spawn_styled(world: &mut World, parent: Option<Entity>, style: Style) -> Entity {
    let e = world.spawn((Widget, style));
    if let Some(p) = parent {
        add_child(world, p, e);
    }
    e
}

fn build_form_app() -> (TestApp, Entity, Entity, Entity) {
    let surface = FramebufSurface::with_format(
        FB_W,
        FB_H,
        ColorFormat::RGB565Swapped,
        ignore_flush as fn(&[u8], PhysicalRect),
    );
    let mut app = App::new(surface);
    app.with_default_widgets()
        .with_offscreen_pool_budget(64 * 1024);
    let world = &mut app.world;

    let form_page = spawn_styled(
        world,
        None,
        Style {
            bg_color: Some(ThemedColor::Token(ColorToken::Surface)),
            layout: LayoutStyle {
                width: Dimension::Px(Fixed::from_int(128)),
                height: Dimension::Px(Fixed::from_int(114)),
                direction: FlexDirection::Column,
                padding: Padding::all(Dimension::Px(Fixed::from_int(10))),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    world.insert(form_page, OffscreenRender::default());

    let enable_row = spawn_styled(
        world,
        Some(form_page),
        Style {
            layout: LayoutStyle {
                direction: FlexDirection::Row,
                height: Dimension::Px(Fixed::from_int(28)),
                align: AlignItems::Center,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let enable_label = spawn_styled(
        world,
        Some(enable_row),
        Style {
            text_color: ThemedColor::Token(ColorToken::OnSurface),
            layout: LayoutStyle {
                grow: Fixed::ONE,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    world.insert(enable_label, Text::new("Enable"));
    let enable_switch = spawn_styled(
        world,
        Some(enable_row),
        Style {
            layout: LayoutStyle {
                width: Dimension::Px(Fixed::from_int(40)),
                height: Dimension::Px(Fixed::from_int(20)),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    world.insert(enable_switch, Switch::new());

    let slider_row = spawn_styled(
        world,
        Some(form_page),
        Style {
            layout: LayoutStyle {
                height: Dimension::Px(Fixed::from_int(14)),
                padding: Padding {
                    top: Dimension::Px(Fixed::from_int(6)),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let value_slider = spawn_styled(
        world,
        Some(slider_row),
        Style {
            layout: LayoutStyle {
                width: Dimension::Px(Fixed::from_int(108)),
                height: Dimension::Px(Fixed::from_int(14)),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    world.insert(value_slider, Slider::new(Fixed::ZERO, Fixed::from_int(100)));

    let progress_row = spawn_styled(
        world,
        Some(form_page),
        Style {
            layout: LayoutStyle {
                height: Dimension::Px(Fixed::from_int(10)),
                padding: Padding {
                    top: Dimension::Px(Fixed::from_int(8)),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let value_progress = spawn_styled(
        world,
        Some(progress_row),
        Style {
            border_radius: Fixed::from_int(4),
            layout: LayoutStyle {
                width: Dimension::Px(Fixed::from_int(108)),
                height: Dimension::Px(Fixed::from_int(8)),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    world.insert(value_progress, ProgressBar::new());

    app.set_root(form_page);
    (app, form_page, enable_switch, value_slider)
}

const B64_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(input: &[u8], out: &mut [u8]) -> usize {
    let mut o = 0;
    let mut i = 0;
    while i + 3 <= input.len() {
        let b0 = input[i];
        let b1 = input[i + 1];
        let b2 = input[i + 2];
        out[o] = B64_ALPHABET[(b0 >> 2) as usize];
        out[o + 1] = B64_ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize];
        out[o + 2] = B64_ALPHABET[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize];
        out[o + 3] = B64_ALPHABET[(b2 & 0x3F) as usize];
        o += 4;
        i += 3;
    }
    let rem = input.len() - i;
    if rem == 1 {
        let b0 = input[i];
        out[o] = B64_ALPHABET[(b0 >> 2) as usize];
        out[o + 1] = B64_ALPHABET[((b0 & 0x03) << 4) as usize];
        out[o + 2] = b'=';
        out[o + 3] = b'=';
        o += 4;
    } else if rem == 2 {
        let b0 = input[i];
        let b1 = input[i + 1];
        out[o] = B64_ALPHABET[(b0 >> 2) as usize];
        out[o + 1] = B64_ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize];
        out[o + 2] = B64_ALPHABET[((b1 & 0x0F) << 2) as usize];
        out[o + 3] = b'=';
        o += 4;
    }
    o
}

/// FNV-1a hash of the framebuffer bytes.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn dump(label: &str, bytes: &[u8], report: &mut impl FnMut(Arguments<'_>)) {
    report(format_args!(
        "[TEST] {} fnv1a64=0x{:016x}",
        label,
        fnv1a_64(bytes)
    ));
    report(format_args!("[FB_BEGIN] frame={} len={}", label, FB_BYTES));
    let mut out = [0u8; 64];
    for chunk in bytes.chunks(48) {
        let n = b64_encode(chunk, &mut out);
        report(format_args!(
            "[FB] {}",
            core::str::from_utf8(&out[..n]).expect("base64")
        ));
    }
    report(format_args!("[FB_END] frame={}", label));
}

fn render_and_dump(label: &str, app: &mut TestApp, report: &mut impl FnMut(Arguments<'_>)) -> u64 {
    app.render().expect("prepare and render diagnostic frame");
    let texture = app.backend.framebuffer();
    let bytes = texture.buf.as_slice();
    assert!(
        bytes.iter().any(|byte| *byte != 0),
        "empty diagnostic frame"
    );
    dump(label, bytes, report);
    fnv1a_64(bytes)
}

fn clipped_hash(bytes: &[u8], area: Rect) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for y in 0..FB_H as i32 {
        for x in 0..FB_W as i32 {
            let inside = x >= area.x.to_int()
                && x < (area.x + area.w).to_int()
                && y >= area.y.to_int()
                && y < (area.y + area.h).to_int();
            let offset = (y as usize * FB_W as usize + x as usize) * 2;
            for byte in &bytes[offset..offset + 2] {
                hash ^= if inside { *byte as u64 } else { 0 };
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
    }
    hash
}

fn render_region_and_check(
    label: &str,
    app: &mut TestApp,
    root: Entity,
    area: Rect,
    report: &mut impl FnMut(Arguments<'_>),
) {
    app.render().expect("prepare region reference frame");
    let expected = clipped_hash(app.backend.framebuffer().buf.as_slice(), area);
    let viewport = app.backend.viewport();
    let mut texture = app.backend.framebuffer();
    texture.buf.as_mut_slice().fill(0);
    {
        let mut renderer = SwRenderer::new(texture);
        render_system::render_region(&app.world, root, &viewport, &area, &mut renderer)
            .expect("render diagnostic region");
    }
    let texture = app.backend.framebuffer();
    assert_eq!(
        fnv1a_64(texture.buf.as_slice()),
        expected,
        "clipped pixels differ from full frame"
    );
    dump(label, texture.buf.as_slice(), report);
}

/// Emit deterministic frames without touching display peripherals.
pub fn run_frames(mut report: impl FnMut(Arguments<'_>)) {
    report(format_args!("[TEST] esp-test-offscreen starting"));
    {
        let (mut app, root, switch, slider) = build_form_app();
        let off = render_and_dump("form_off", &mut app, &mut report);
        app.world.get_mut::<Switch>(switch).unwrap().on = true;
        app.world.insert(switch, Dirty);
        let on = render_and_dump("form_on", &mut app, &mut report);
        assert_ne!(off, on, "switch state did not repaint");
        let slider_state = app.world.get_mut::<Slider>(slider).unwrap();
        slider_state.value = slider_state.max;
        app.world.insert(slider, Dirty);
        let max = render_and_dump("form_slider_max", &mut app, &mut report);
        assert_ne!(on, max, "slider state did not repaint");
        render_region_and_check(
            "form_region_full",
            &mut app,
            root,
            Rect::new(0, 0, 128, 128),
            &mut report,
        );
        render_region_and_check(
            "form_region_switch",
            &mut app,
            root,
            Rect::new(78, 10, 40, 20),
            &mut report,
        );
    }
    {
        let (mut app, root, switch, _) = build_form_app();
        app.world.remove::<OffscreenRender>(root);
        app.world.insert(switch, OffscreenRender::default());
        let off = render_and_dump("switch_only_off", &mut app, &mut report);
        app.world.get_mut::<Switch>(switch).unwrap().on = true;
        app.world.insert(switch, Dirty);
        let on = render_and_dump("switch_only_on", &mut app, &mut report);
        assert_ne!(off, on, "captured switch state did not repaint");
        render_region_and_check(
            "switch_only_region",
            &mut app,
            root,
            Rect::new(78, 10, 40, 20),
            &mut report,
        );
    }
    report(format_args!("[TEST] done"));
}
