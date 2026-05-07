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

use mirui::components::assets::*;
use mirui::components::image::Image;
use mirui::draw::{Renderer, SwRenderer};
use mirui::ecs::{Entity, World};
use mirui::layout::*;
use mirui::types::Color;
use mirui::widget::builder::WidgetBuilder;
use mirui::widget::render_system;
use mirui::widget::Style;

esp_bootloader_esp_idf::esp_app_desc!();

const W: u16 = 128;
const H: u16 = 128;

// === Components ===
struct Velocity { vx: i32, vy: i32 }
struct PhysicsBody { x: i32, y: i32 }
struct FrameCounter(u32);
struct BodyEntities([Entity; 3]);

// === Systems ===
fn three_body_system(world: &mut World) {
    const EQUILIBRIUM: i32 = 30;

    for _ in 0..4 {

    let entities = match world.resource::<BodyEntities>() {
        Some(b) => b.0,
        None => return,
    };

    let mut positions = [(0i32, 0i32); 3];
    for i in 0..3 {
        if let Some(body) = world.get::<PhysicsBody>(entities[i]) {
            positions[i] = (body.x, body.y);
        }
    }

    let mut ax = [0i32; 3];
    let mut ay = [0i32; 3];
    for i in 0..3 {
        for j in (i+1)..3 {
            let dx = positions[j].0 - positions[i].0;
            let dy = positions[j].1 - positions[i].1;
            let dist = isqrt(((dx/256)*(dx/256) + (dy/256)*(dy/256)) as u32) as i32;
            if dist == 0 { continue; }
            let force = 60 * (dist - EQUILIBRIUM) / dist.max(1);
            let fx = (force * (dx / 256)) / dist;
            let fy = (force * (dy / 256)) / dist;
            ax[i] += fx; ay[i] += fy;
            ax[j] -= fx; ay[j] -= fy;
        }
    }

    for i in 0..3 {
        let e = entities[i];
        if let Some(vel) = world.get_mut::<Velocity>(e) {
            vel.vx += ax[i];
            vel.vy += ay[i];
            vel.vx = vel.vx.clamp(-600, 600);
            vel.vy = vel.vy.clamp(-600, 600);
        }
        let (vx, vy) = world.get::<Velocity>(e).map(|v| (v.vx, v.vy)).unwrap_or((0,0));
        if let Some(body) = world.get_mut::<PhysicsBody>(e) {
            body.x += vx;
            body.y += vy;
            let min = 8 * 256;
            let max_x = (W as i32 - 8) * 256;
            let max_y = (H as i32 - 8) * 256;
            if body.x < min { body.x = min; }
            if body.x > max_x { body.x = max_x; }
            if body.y < min { body.y = min; }
            if body.y > max_y { body.y = max_y; }
        }
        if let Some(body) = world.get::<PhysicsBody>(e) {
            let bx = body.x; let by = body.y;
            if let Some(vel) = world.get_mut::<Velocity>(e) {
                if bx <= 8*256 || bx >= (W as i32 -8)*256 { vel.vx = -vel.vx; }
                if by <= 8*256 || by >= (H as i32 -8)*256 { vel.vy = -vel.vy; }
            }
        }
    }
    } // end for _ in 0..4
}

fn kick_system(world: &mut World) {
    let fc = world.resource::<FrameCounter>().map(|f| f.0).unwrap_or(0);
    let entities = match world.resource::<BodyEntities>() {
        Some(b) => b.0,
        None => return,
    };
    if fc % 40 == 0 {
        let kick_idx = (fc / 40) as usize % 3;
        let kick_dir = (fc / 120) as i32;
        let e = entities[kick_idx];
        if let Some(vel) = world.get_mut::<Velocity>(e) {
            vel.vx += ((kick_dir * 7) % 13 - 6) * 80;
            vel.vy += ((kick_dir * 11) % 13 - 6) * 80;
        }
    }
}

fn sync_layout_system(world: &mut World) {
    let iw = IMG_THUMBS_UP_WIDTH as i32;
    let ih = IMG_THUMBS_UP_HEIGHT as i32;
    let entities = match world.resource::<BodyEntities>() {
        Some(b) => b.0,
        None => return,
    };
    for i in 0..3 {
        let e = entities[i];
        let (bx, by) = world.get::<PhysicsBody>(e)
            .map(|b| (b.x / 256 - iw / 2, b.y / 256 - ih / 2))
            .unwrap_or((0, 0));
        mirui::widget::set_position(world, e, bx, by);
    }
}

fn frame_counter_system(world: &mut World) {
    if let Some(fc) = world.resource_mut::<FrameCounter>() {
        fc.0 = fc.0.wrapping_add(1);
    }
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
        self.cmd(0xB1); self.data(&[0x01,0x2C,0x2D]);
        self.cmd(0xB2); self.data(&[0x01,0x2C,0x2D]);
        self.cmd(0xB3); self.data(&[0x01,0x2C,0x2D,0x01,0x2C,0x2D]);
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
    fn push_pixels(&mut self, data: &[u8]) {
        self.set_window(0, 0, W-1, H-1);
        self.cs.set_low(); self.dc.set_high();
        for chunk in data.chunks(512) { self.spi.write(chunk).ok(); }
        self.cs.set_high();
    }
    fn push_region(&mut self, rgba: &[u8], fb_w: u16, x: u16, y: u16, w: u16, h: u16) {
        self.set_window(x, y, x + w - 1, y + h - 1);
        self.cs.set_low(); self.dc.set_high();
        let mut row_buf = vec![0u8; w as usize * 2];
        for row in 0..h as usize {
            for col in 0..w as usize {
                let i = ((y as usize + row) * fb_w as usize + x as usize + col) * 4;
                let r = rgba[i] as u16;
                let g = rgba[i+1] as u16;
                let b = rgba[i+2] as u16;
                let px = ((r>>3)<<11)|((g>>2)<<5)|(b>>3);
                row_buf[col*2] = (px>>8) as u8;
                row_buf[col*2+1] = px as u8;
            }
            self.spi.write(&row_buf).ok();
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

fn systimer_now() -> u32 {
    let val: u32;
    unsafe { core::arch::asm!("csrr {}, 0x7E2", out(reg) val); }
    val
}

fn draw_fps(fb: &mut [u8], fb_w: u16, fps: u32) {
    let mut num = [0u8; 8];
    let mut len = 0;
    let mut n = fps;
    if n == 0 { num[0] = b'0'; len = 1; }
    else { while n > 0 && len < 7 { num[len] = b'0' + (n % 10) as u8; n /= 10; len += 1; } num[..len].reverse(); }
    num[len] = b'f'; len += 1;
    let start_x = fb_w as i32 - (len as i32) * 8 - 2;
    for (ci, &ch) in num[..len].iter().enumerate() {
        let glyph = mirui::draw::font::glyph(ch);
        for row in 0..8i32 {
            let byte = glyph[row as usize];
            for col in 0..8i32 {
                if byte & (0x80 >> col) != 0 {
                    let px = start_x + ci as i32 * 8 + col;
                    let py = 2 + row;
                    if px >= 0 && px < fb_w as i32 && py < fb_w as i32 {
                        let idx = ((py as u32 * fb_w as u32 + px as u32) * 4) as usize;
                        fb[idx] = 255; fb[idx+1] = 255; fb[idx+2] = 0; fb[idx+3] = 255;
                    }
                }
            }
        }
    }
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

    // === ECS Setup ===
    let mut world = World::new();

    // Root entity (id=0) also holds FrameCounter
    let root = WidgetBuilder::new(&mut world)
        .bg_color(Color::rgb(30, 30, 46))
        .layout(LayoutStyle { direction: FlexDirection::Column, width: Some(W), height: Some(H), ..Default::default() })
        .id();
    world.insert_resource(FrameCounter(0));

    // Static UI
    let header = WidgetBuilder::new(&mut world)
        .bg_color(Color::rgb(88, 166, 255)).text("mirui").border_radius(3)
        .layout(LayoutStyle { height: Some(20), ..Default::default() })
        .id();
    let left = WidgetBuilder::new(&mut world)
        .bg_color(Color::rgb(63, 185, 80))
        .layout(LayoutStyle { grow: 1.0, ..Default::default() })
        .id();
    let right = WidgetBuilder::new(&mut world)
        .bg_color(Color::rgb(248, 81, 73))
        .layout(LayoutStyle { grow: 1.0, ..Default::default() })
        .id();
    let row = WidgetBuilder::new(&mut world)
        .layout(LayoutStyle { direction: FlexDirection::Row, grow: 1.0, ..Default::default() })
        .child(left).child(right)
        .id();
    let footer = WidgetBuilder::new(&mut world)
        .bg_color(Color::rgb(210, 168, 255)).text("3-body")
        .layout(LayoutStyle { height: Some(20), ..Default::default() })
        .id();

    // 3 image entities with physics (ids will be 10, 11, 12 — but we can't guarantee that)
    // Instead, store entity ids and use them in systems via a resource component
    let iw = IMG_THUMBS_UP_WIDTH;
    let ih = IMG_THUMBS_UP_HEIGHT;
    let cx = (W as i32 / 2) * 256;
    let cy = (H as i32 / 2) * 256;
    let r = 30 * 256;

    let init_pos = [
        (cx, cy - r, 350i32, 0i32),
        (cx - r * 7 / 8, cy + r / 2, -175, 300),
        (cx + r * 7 / 8, cy + r / 2, -175, -300),
    ];

    let mut img_entities: [Entity; 3] = [Entity { id: 0, generation: 0 }; 3];
    for i in 0..3 {
        let e = WidgetBuilder::new(&mut world)
            .layout(LayoutStyle {
                position: Position::Absolute,
                left: Some(init_pos[i].0 / 256 - iw as i32 / 2),
                top: Some(init_pos[i].1 / 256 - ih as i32 / 2),
                width: Some(iw), height: Some(ih),
                ..Default::default()
            })
            .id();
        world.insert(e, Image::new(Vec::from(IMG_THUMBS_UP), iw, ih));
        world.insert(e, PhysicsBody { x: init_pos[i].0, y: init_pos[i].1 });
        world.insert(e, Velocity { vx: init_pos[i].2, vy: init_pos[i].3 });
        img_entities[i] = e;
    }

    // Build tree
    use mirui::widget::{Children, Parent};
    world.insert_resource(BodyEntities(img_entities));
    for &child in &[header, row, footer, img_entities[0], img_entities[1], img_entities[2]] {
        world.insert(child, Parent(root));
        if let Some(children) = world.get_mut::<Children>(root) {
            children.0.push(child);
        }
    }

    // Initial full render
    let mut rgba = vec![0u8; W as usize * H as usize * 4];
    {
        let mut renderer = SwRenderer::new(&mut rgba, W as u32, H as u32);
        render_system::render(&world, root, W, H, 1, &mut renderer);
    }
    lcd.push_pixels(&{
        let mut buf = vec![0u8; W as usize * H as usize * 2];
        for i in 0..(W as usize * H as usize) {
            let r = rgba[i*4] as u16; let g = rgba[i*4+1] as u16; let b = rgba[i*4+2] as u16;
            let px = ((r>>3)<<11)|((g>>2)<<5)|(b>>3);
            buf[i*2] = (px>>8) as u8; buf[i*2+1] = px as u8;
        }
        buf
    });

    // === Main Loop (manual, since no App on bare metal) ===
    // System scheduler
    let mut scheduler = mirui::ecs::SystemScheduler::new();
    scheduler.add(three_body_system);
    scheduler.add(kick_system);
    scheduler.add(frame_counter_system);

    let mut fps_display: u32 = 0;
    let mut fps_count: u32 = 0;
    let mut last_time = systimer_now();

    loop {
        fps_count += 1;
        let now = systimer_now();
        if now.wrapping_sub(last_time) >= 160_000_000 {
            fps_display = fps_count;
            fps_count = 0;
            last_time = now;
        }

        // Run systems via scheduler
        scheduler.run_all(&mut world);
        sync_layout_system(&mut world);

        // Single collect — PrevRect handles old+new automatically
        let dirty = render_system::collect_dirty_region(&mut world, root, W, H, 1);

        if let Some(dr) = dirty {
            let dx = (dr.x.max(0) as u16).min(W - 1);
            let dy = (dr.y.max(0) as u16).min(H - 1);
            let dw = dr.w.min(W - dx);
            let dh = dr.h.min(H - dy);
            if dw > 0 && dh > 0 {
                let clip = mirui::types::Rect { x: dx as i32, y: dy as i32, w: dw, h: dh };
                let mut renderer = SwRenderer::new(&mut rgba, W as u32, H as u32);
                render_system::render_region(&world, root, W, H, 1, &clip, &mut renderer);
                lcd.push_region(&rgba, W, dx, dy, dw, dh);
            }
        }

        // FPS overlay
        draw_fps(&mut rgba, W, fps_display);
        lcd.push_region(&rgba, W, W - 50, 0, 50, 10);
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
