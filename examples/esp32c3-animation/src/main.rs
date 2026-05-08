#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use esp_alloc as _;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::spi::Mode as SpiMode;
use esp_hal::time::Rate;

use mirui::app::App;
use mirui::backend::framebuf::FramebufBackend;
use mirui::components::assets::*;
use mirui::components::image::Image;
use mirui::ecs::{Entity, World};
use mirui::layout::*;
use mirui::types::{Color, Dimension, Fixed, Rect};
use mirui::widget::builder::WidgetBuilder;

esp_bootloader_esp_idf::esp_app_desc!();

const W: u16 = 128;
const H: u16 = 128;

// === Components ===
struct Velocity { vx: Fixed, vy: Fixed }
struct PhysicsBody { x: Fixed, y: Fixed }
struct FrameCounter(u32);
struct FpsState { count: u32, last_tick: u32, display: u32 }
struct PhysicsTime { last_tick: u32, accumulator: u32 }
const PHYSICS_DT: u32 = 1_111_111; // 160MHz / 144 = ~1.11M ticks = 6.94ms fixed step (144Hz)

// === Systems ===
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
        for j in (i+1)..3 {
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
    let fc = world.resource::<FrameCounter>().map(|f| f.0).unwrap_or(0);
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

fn frame_counter_system(world: &mut World) {
    if let Some(fc) = world.resource_mut::<FrameCounter>() {
        fc.0 = fc.0.wrapping_add(1);
    }
}

static mut FPS_DISPLAY: u32 = 0;

fn fps_system(world: &mut World) {
    let now = systimer_now();
    let Some(fps) = world.resource_mut::<FpsState>() else { return };
    fps.count += 1;
    if now.wrapping_sub(fps.last_tick) >= 160_000_000 {
        fps.display = fps.count;
        fps.count = 0;
        fps.last_tick = now;
        unsafe { FPS_DISPLAY = fps.display; }
    }
}

fn systimer_now() -> u32 {
    let val: u32;
    unsafe { core::arch::asm!("csrr {}, 0x7E2", out(reg) val); }
    val
}

// === LCD Driver ===
struct St7735<'a> {
    spi: Spi<'a, esp_hal::Blocking>,
    dc: Output<'a>,
    cs: Output<'a>,
}
impl<'a> St7735<'a> {
    fn cmd(&mut self, c: u8) { self.cs.set_low(); self.dc.set_low(); self.spi.write(&[c]).ok(); self.cs.set_high(); }
    fn data(&mut self, d: &[u8]) { self.cs.set_low(); self.dc.set_high(); self.spi.write(d).ok(); self.cs.set_high(); }
    fn init(&mut self, rst: &mut Output) {
        rst.set_low(); delay_ms(10); rst.set_high(); delay_ms(120);
        self.cmd(0x01); delay_ms(150); self.cmd(0x11); delay_ms(500);
        self.cmd(0xB1); self.data(&[0x01,0x01,0x01]);
        self.cmd(0xB2); self.data(&[0x01,0x01,0x01]);
        self.cmd(0xB3); self.data(&[0x01,0x01,0x01,0x01,0x01,0x01]);
        self.cmd(0xB4); self.data(&[0x07]);
        self.cmd(0xC0); self.data(&[0xA2,0x02,0x84]);
        self.cmd(0xC1); self.data(&[0xC5]);
        self.cmd(0xC2); self.data(&[0x0A,0x00]);
        self.cmd(0xC3); self.data(&[0x8A,0x2A]);
        self.cmd(0xC4); self.data(&[0x8A,0xEE]);
        self.cmd(0xC5); self.data(&[0x0E]);
        self.cmd(0x20); self.cmd(0x36); self.data(&[0xC8]);
        self.cmd(0x3A); self.data(&[0x05]);
        self.cmd(0x13); delay_ms(10); self.cmd(0x29); delay_ms(100);
    }
    fn set_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        let (xo, yo) = (2u16, 3u16);
        self.cmd(0x2A); self.data(&[((x0+xo)>>8) as u8,(x0+xo) as u8,((x1+xo)>>8) as u8,(x1+xo) as u8]);
        self.cmd(0x2B); self.data(&[((y0+yo)>>8) as u8,(y0+yo) as u8,((y1+yo)>>8) as u8,(y1+yo) as u8]);
        self.cmd(0x2C);
    }
    fn push_region(&mut self, rgba: &[u8], fb_w: u16, x: u16, y: u16, w: u16, h: u16) {
        const BATCH_ROWS: usize = 16;
        self.set_window(x, y, x + w - 1, y + h - 1);
        self.cs.set_low(); self.dc.set_high();
        let mut buf = vec![0u8; w as usize * 2 * BATCH_ROWS];
        let mut row = 0usize;
        while row < h as usize {
            let rows_this_batch = BATCH_ROWS.min(h as usize - row);
            let buf_len = w as usize * 2 * rows_this_batch;
            for r in 0..rows_this_batch {
                for col in 0..w as usize {
                    let i = ((y as usize + row + r) * fb_w as usize + x as usize + col) * 4;
                    let rv = rgba[i] as u16;
                    let gv = rgba[i+1] as u16;
                    let bv = rgba[i+2] as u16;
                    let px = ((rv>>3)<<11)|((gv>>2)<<5)|(bv>>3);
                    let off = (r * w as usize + col) * 2;
                    buf[off] = (px>>8) as u8;
                    buf[off+1] = px as u8;
                }
            }
            self.spi.write(&buf[..buf_len]).ok();
            row += rows_this_batch;
        }
        self.cs.set_high();
    }
}

fn delay_ms(ms: u32) { for _ in 0..ms { for _ in 0..16_000u32 { core::hint::spin_loop(); } } }

fn isqrt(n: u32) -> u32 {
    if n == 0 { return 0; }
    let mut x = n; let mut y = (x + 1) / 2;
    while y < x { x = y; y = (x + n / x) / 2; } x
}

fn draw_fps_lcd(lcd: &mut St7735, fps: u32) {
    let mut num = [0u8; 8];
    let mut len = 0;
    let mut n = fps;
    if n == 0 { num[0] = b'0'; len = 1; }
    else { while n > 0 && len < 5 { num[len] = b'0' + (n % 10) as u8; n /= 10; len += 1; } num[..len].reverse(); }
    num[len] = b'f'; len += 1;

    let fw: u16 = (len as u16) * 8;
    let fh: u16 = 8;
    let sx = W - fw - 2;
    lcd.set_window(sx, 2, sx + fw - 1, 2 + fh - 1);
    lcd.cs.set_low(); lcd.dc.set_high();
    let mut row_buf = vec![0u8; fw as usize * 2];
    for row in 0..fh as usize {
        for col in 0..fw as usize {
            let ci = col / 8;
            let bit = col % 8;
            let glyph = mirui::draw::font::glyph(num[ci]);
            let on = glyph[row] & (0x80 >> bit) != 0;
            let px: u16 = if on { 0xFFE0 } else { 0x0000 }; // yellow on black
            row_buf[col*2] = (px>>8) as u8; row_buf[col*2+1] = px as u8;
        }
        lcd.spi.write(&row_buf).ok();
    }
    lcd.cs.set_high();
}

// === Main ===
#[esp_hal::main]
fn main() -> ! {
    esp_alloc::heap_allocator!(size: 200 * 1024);
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let spi = Spi::new(peripherals.SPI2, SpiConfig::default().with_frequency(Rate::from_mhz(26)).with_mode(SpiMode::_0)).unwrap().with_sck(peripherals.GPIO5).with_mosi(peripherals.GPIO4);
    let cs = Output::new(peripherals.GPIO6, Level::High, OutputConfig::default());
    let dc = Output::new(peripherals.GPIO7, Level::Low, OutputConfig::default());
    let mut rst = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());
    let mut bl = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());
    let mut lcd = St7735 { spi, dc, cs };
    lcd.init(&mut rst);
    bl.set_high();

    // Backend: flush callback pushes dirty region to LCD + FPS overlay
    let backend = FramebufBackend::new(W, H, move |buf: &[u8], area: &Rect| {
        let x = area.x.max(0) as u16;
        let y = area.y.max(0) as u16;
        let w = area.w.min(W - x);
        let h = area.h.min(H - y);
        if w > 0 && h > 0 {
            lcd.push_region(buf, W, x, y, w, h);
        }
        // FPS: draw directly to LCD at top-right, independent of dirty rect
        let fps = unsafe { FPS_DISPLAY };
        draw_fps_lcd(&mut lcd, fps);
    });

    let mut app = App::new(backend);

    // Systems
    app.add_system(physics_tick_system);
    app.add_system(kick_system);
    app.add_system(sync_layout_system);
    app.add_system(frame_counter_system);
    app.add_system(fps_system);

    // UI setup
    let world = &mut app.world;
    world.insert_resource(FrameCounter(0));

    // Static UI via DSL
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

    // Dynamic image entities with physics
    let iw = IMG_THUMBS_UP_WIDTH;
    let ih = IMG_THUMBS_UP_HEIGHT;
    let cx = Fixed::from_int(W as i32 / 2);
    let cy = Fixed::from_int(H as i32 / 2);
    let r = Fixed::from_int(30);
    let r78 = r * 7 / 8; // r * 7/8
    let init_pos = [
        (cx, cy - r, Fixed::from_raw(350), Fixed::ZERO),
        (cx - r78, cy + r / 2, Fixed::from_raw(-175), Fixed::from_raw(300)),
        (cx + r78, cy + r / 2, Fixed::from_raw(-175), Fixed::from_raw(-300)),
    ];

    // Create physics bodies via DSL with enchants
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

    world.insert_resource(FpsState { count: 0, last_tick: systimer_now(), display: 0 });
    world.insert_resource(PhysicsTime { last_tick: systimer_now(), accumulator: 0 });

    app.set_root(root);
    app.render();
    app.run();
    loop {}
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
