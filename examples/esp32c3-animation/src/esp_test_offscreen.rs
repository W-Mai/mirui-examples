//! Diagnostic harness for OffscreenRender ESP-vs-desktop bisect.
//!
//! Renders a sequence of fixtures (form_page subtree with the demo's
//! tab-B widgets, plus per-state variants flipping Switch on or
//! Slider to its max) into a 128×128 RGB565Swapped framebuffer with
//! OffscreenRender placed in different positions, dumps each frame
//! as base64 over UART, and prints an FNV-1a hash per frame. A
//! matching desktop unit test runs the same fixtures and compares
//! hashes; divergence on a specific frame names the path that
//! differs between platforms.
//!
//! Why a separate harness rather than the demo loop:
//! - No SimTimeline, no input, no FrameStats: the variables that
//!   could shift between ESP and desktop are reduced to format,
//!   target arch, and opt level.
//! - Each frame is a full `render_system::render` (or `render_region`)
//!   call followed by a hash + dump, then halt — no flush callback
//!   timing, no double-buffer race, no dirty-region masking.

use alloc::vec::Vec;
use mirui::ui::widgets::{ProgressBar, Slider, Switch, Text};
use mirui::render::sw::SwRenderer;
use mirui::render::texture::{ColorFormat, Texture};
use mirui::ecs::{Entity, World};
use mirui::ui::layout::{AlignItems, FlexDirection, LayoutStyle, Padding};
use mirui::types::{Dimension, Fixed, Viewport};
use mirui::ui::dirty::Dirty;
use mirui::ui::theme::{ColorToken, ThemedColor};
use mirui::ui::view::ViewRegistry;
use mirui::ui::{
    Children, OffscreenBufferPool, OffscreenRender, Parent, Style, Theme, Widget, render_system,
};

const FB_W: u16 = 128;
const FB_H: u16 = 128;
const FB_BYTES: usize = FB_W as usize * FB_H as usize * 2;

fn add_child(world: &mut World, parent: Entity, child: Entity) {
    world.insert(child, Parent(parent));
    if let Some(c) = world.get_mut::<Children>(parent) {
        c.0.push(child);
    } else {
        world.insert(parent, Children(alloc::vec![child]));
    }
}

fn spawn_styled(world: &mut World, parent: Option<Entity>, style: Style) -> Entity {
    let e = world.spawn();
    world.insert(e, Widget);
    world.insert(e, style);
    if let Some(p) = parent {
        add_child(world, p, e);
    }
    e
}

/// Build a form_page subtree mirroring `demo_widgets::setup`'s tab B,
/// with OffscreenRender on form_page itself. No SimTimeline, no
/// gesture state — just static layout. Returns
/// (world, form_page, enable_switch, value_slider).
fn build_form_world() -> (World, Entity, Entity, Entity) {
    let mut world = World::default();
    // ESP heap is 200 KB; a 128×114 RGB565 form panel buffer is
    // ~29 KB, so 64 KiB fits roughly two of them.
    world.insert_resource(OffscreenBufferPool::with_budget(64 * 1024));
    world.insert_resource(ViewRegistry::with_builtins());
    world.insert_resource(Theme::dark());

    let form_page = spawn_styled(
        &mut world,
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
        &mut world,
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
        &mut world,
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
    world.insert(enable_label, Text(b"Enable".to_vec()));
    let enable_switch = spawn_styled(
        &mut world,
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
        &mut world,
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
        &mut world,
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
        &mut world,
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
        &mut world,
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

    (world, form_page, enable_switch, value_slider)
}

const B64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

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

/// FNV-1a 64-bit. Cheap deterministic hash for the framebuffer that
/// matches what the desktop test computes.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Render one frame and emit a hash + base64 dump tagged with the
/// frame label. Splitting per-frame output keeps the desktop
/// comparison surgical: the first hash that diverges names the
/// faulty path.
fn render_and_dump(
    label: &str,
    world: &World,
    root: Entity,
    viewport: &Viewport,
    fb: &mut [u8],
) {
    // Wipe the framebuffer between frames. mirui doesn't auto-clear
    // because the host backend usually owns the present cycle; in this
    // harness we want each frame's hash to reflect *that* frame's
    // contribution alone.
    fb.fill(0);

    let tex = Texture::new(fb, FB_W, FB_H, ColorFormat::RGB565Swapped);
    let mut renderer = SwRenderer::new(tex);
    render_system::render(world, root, viewport, &mut renderer);

    let hash = fnv1a_64(fb);
    esp_println::println!("[TEST] {} fnv1a64=0x{:016x}", label, hash);

    esp_println::println!("[FB_BEGIN] frame={} len={}", label, FB_BYTES);
    const CHUNK: usize = 48;
    let mut idx = 0;
    let mut out = [0u8; 64 + 4];
    while idx < fb.len() {
        let end = (idx + CHUNK).min(fb.len());
        let n = b64_encode(&fb[idx..end], &mut out);
        let s = core::str::from_utf8(&out[..n]).unwrap_or("");
        esp_println::println!("[FB] {}", s);
        idx = end;
    }
    esp_println::println!("[FB_END] frame={}", label);
}

pub fn run() -> ! {
    esp_println::println!("[TEST] esp-test-offscreen starting");

    let (mut world, form_page, enable_switch, value_slider) = build_form_world();
    esp_println::println!("[TEST] form_page world built, root={:?}", form_page);

    let mut fb: Vec<u8> = alloc::vec![0u8; FB_BYTES];
    let viewport = Viewport::new(FB_W, FB_H, Fixed::ONE);

    esp_println::println!(
        "[TEST] fb {}x{} fmt=RGB565Swapped bytes={} viewport.scale=1.0",
        FB_W,
        FB_H,
        FB_BYTES
    );

    // Frame variant 1 — pristine state: switch off, slider min.
    render_and_dump("form_off", &world, form_page, &viewport, &mut fb);

    // Frame variant 2 — switch flipped on. With no SwitchBgT/AnimatedThumbX
    // present, switch_render falls back to the on-endpoint (t = 1.0) so
    // this exercises the on-state colour path without involving any
    // tween system. Mark Dirty so the dirty walker bumps generation
    // and the buffer pool re-rasters instead of returning the cached
    // off-state buffer.
    if let Some(sw) = world.get_mut::<Switch>(enable_switch) {
        sw.on = true;
    }
    world.insert(enable_switch, Dirty);
    render_and_dump("form_on", &world, form_page, &viewport, &mut fb);

    // Frame variant 3 — slider at max (ratio 1.0). Exercises the fill
    // bar path that's clipped to ratio_w.
    if let Some(s) = world.get_mut::<Slider>(value_slider) {
        s.value = s.max;
    }
    world.insert(value_slider, Dirty);
    render_and_dump("form_slider_max", &world, form_page, &viewport, &mut fb);

    // Frame variant 4 — render_region with a tight dirty rect on
    // the switch only (40×20 in the form's coordinate space). This
    // mirrors what App::render_dirty does after a single Dirty marker.
    {
        use mirui::types::Rect;
        fb.fill(0);
        let tex = Texture::new(&mut fb, FB_W, FB_H, ColorFormat::RGB565Swapped);
        let mut renderer = SwRenderer::new(tex);
        let dirty = Rect::new(0, 0, 128, 128);
        render_system::render_region(&world, form_page, &viewport, &dirty, &mut renderer);
        let h = fnv1a_64(&fb);
        esp_println::println!("[TEST] form_region_full fnv1a64=0x{:016x}", h);
    }
    {
        use mirui::types::Rect;
        fb.fill(0);
        let tex = Texture::new(&mut fb, FB_W, FB_H, ColorFormat::RGB565Swapped);
        let mut renderer = SwRenderer::new(tex);
        let dirty = Rect::new(78, 10, 40, 20);
        render_system::render_region(&world, form_page, &viewport, &dirty, &mut renderer);
        let h = fnv1a_64(&fb);
        esp_println::println!("[TEST] form_region_switch fnv1a64=0x{:016x}", h);
    }

    // -------- Variant family 2: OffscreenRender on Switch alone --------
    // Mirrors the demo's current state where enable_switch (not
    // form_page) carries OffscreenRender. Expected: identical pixel
    // result to inline since Switch's render is fully encapsulated.
    {
        let (mut w2, fp2, sw2, _) = build_form_world();
        // Move OffscreenRender from form_page → enable_switch.
        w2.remove::<OffscreenRender>(fp2);
        w2.insert(sw2, OffscreenRender::default());

        render_and_dump("switch_only_off", &w2, fp2, &viewport, &mut fb);

        if let Some(s) = w2.get_mut::<Switch>(sw2) {
            s.on = true;
        }
        w2.insert(sw2, Dirty);
        render_and_dump("switch_only_on", &w2, fp2, &viewport, &mut fb);

        // render_region with tight switch dirty rect — this is the
        // exact path App::render_dirty takes after a switch tap.
        {
            use mirui::types::Rect;
            fb.fill(0);
            let tex = Texture::new(&mut fb, FB_W, FB_H, ColorFormat::RGB565Swapped);
            let mut renderer = SwRenderer::new(tex);
            let dirty = Rect::new(78, 10, 40, 20);
            render_system::render_region(&w2, fp2, &viewport, &dirty, &mut renderer);
            let h = fnv1a_64(&fb);
            esp_println::println!("[TEST] switch_only_region fnv1a64=0x{:016x}", h);
            esp_println::println!("[FB_BEGIN] frame=switch_only_region len={}", FB_BYTES);
            const CHUNK: usize = 48;
            let mut idx = 0;
            let mut out = [0u8; 64 + 4];
            while idx < fb.len() {
                let end = (idx + CHUNK).min(fb.len());
                let n = b64_encode(&fb[idx..end], &mut out);
                let s = core::str::from_utf8(&out[..n]).unwrap_or("");
                esp_println::println!("[FB] {}", s);
                idx = end;
            }
            esp_println::println!("[FB_END] frame=switch_only_region");
        }
    }

    esp_println::println!("[TEST] done — halting");
    loop {
        core::hint::spin_loop();
    }
}
