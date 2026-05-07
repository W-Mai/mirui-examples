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

fn systimer_now() -> u32 {
    let val: u32;
    unsafe { core::arch::asm!("csrr {}, 0x7E2", out(reg) val); }
    val
}

fn isqrt(n: u32) -> u32 {
    if n == 0 { return 0; }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x { x = y; y = (x + n / x) / 2; }
    x
}

struct Body { x: i32, y: i32, vx: i32, vy: i32 }

fn three_body_step(bodies: &mut [Body; 3]) {
    const EQUILIBRIUM: i32 = 30;
    let mut ax = [0i32; 3];
    let mut ay = [0i32; 3];

    for i in 0..3 {
        for j in (i+1)..3 {
            let dx = bodies[j].x - bodies[i].x;
            let dy = bodies[j].y - bodies[i].y;
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
        bodies[i].vx += ax[i];
        bodies[i].vy += ay[i];
        let max_v: i32 = 600;
        bodies[i].vx = bodies[i].vx.clamp(-max_v, max_v);
        bodies[i].vy = bodies[i].vy.clamp(-max_v, max_v);
        bodies[i].x += bodies[i].vx;
        bodies[i].y += bodies[i].vy;

        let min = 8 * 256;
        let max_x = (W as i32 - 8) * 256;
        let max_y = (H as i32 - 8) * 256;
        if bodies[i].x < min { bodies[i].x = min; bodies[i].vx = bodies[i].vx.abs(); }
        if bodies[i].x > max_x { bodies[i].x = max_x; bodies[i].vx = -bodies[i].vx.abs(); }
        if bodies[i].y < min { bodies[i].y = min; bodies[i].vy = bodies[i].vy.abs(); }
        if bodies[i].y > max_y { bodies[i].y = max_y; bodies[i].vy = -bodies[i].vy.abs(); }
    }
}

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

    // Build UI with ECS
    let mut world = World::new();

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

    // 3 image widgets with absolute positioning
    let iw = IMG_THUMBS_UP_WIDTH;
    let ih = IMG_THUMBS_UP_HEIGHT;
    let mut img_entities = [Entity { id: 0, generation: 0 }; 3];
    for i in 0..3 {
        let e = WidgetBuilder::new(&mut world)
            .layout(LayoutStyle {
                position: Position::Absolute,
                left: Some(56),
                top: Some(56),
                width: Some(iw),
                height: Some(ih),
                ..Default::default()
            })
            .id();
        world.insert(e, Image::new(Vec::from(IMG_THUMBS_UP), iw, ih));
        img_entities[i] = e;
    }

    let root = WidgetBuilder::new(&mut world)
        .bg_color(Color::rgb(30, 30, 46))
        .layout(LayoutStyle { direction: FlexDirection::Column, width: Some(W), height: Some(H), ..Default::default() })
        .child(header).child(row).child(footer)
        .child(img_entities[0]).child(img_entities[1]).child(img_entities[2])
        .id();

    // Initial full render
    let mut rgba = vec![0u8; W as usize * H as usize * 4];
    let mut renderer = SwRenderer::new(&mut rgba, W as u32, H as u32);
    render_system::render(&world, root, W, H, 1, &mut renderer);
    lcd.push_pixels(&{
        let mut buf = vec![0u8; W as usize * H as usize * 2];
        for i in 0..(W as usize * H as usize) {
            let r = rgba[i*4] as u16; let g = rgba[i*4+1] as u16; let b = rgba[i*4+2] as u16;
            let px = ((r>>3)<<11)|((g>>2)<<5)|(b>>3);
            buf[i*2] = (px>>8) as u8; buf[i*2+1] = px as u8;
        }
        buf
    });

    // Physics init
    let cx = (W as i32 / 2) * 256;
    let cy = (H as i32 / 2) * 256;
    let r = 30 * 256;
    let mut bodies = [
        Body { x: cx, y: cy - r, vx: 350, vy: 0 },
        Body { x: cx - r * 7 / 8, y: cy + r / 2, vx: -175, vy: 300 },
        Body { x: cx + r * 7 / 8, y: cy + r / 2, vx: -175, vy: -300 },
    ];

    let mut frame: u32 = 0;
    let mut fps_display: u32 = 0;
    let mut fps_count: u32 = 0;
    let mut last_time = systimer_now();

    loop {
        frame = frame.wrapping_add(1);
        fps_count += 1;
        let now = systimer_now();
        if now.wrapping_sub(last_time) >= 160_000_000 {
            fps_display = fps_count;
            fps_count = 0;
            last_time = now;
        }

        // Physics step
        for _ in 0..4 { three_body_step(&mut bodies); }
        if frame % 40 == 0 {
            let kick_idx = (frame / 40) as usize % 3;
            let kick_dir = (frame / 120) as i32;
            bodies[kick_idx].vx += ((kick_dir * 7) % 13 - 6) * 80;
            bodies[kick_idx].vy += ((kick_dir * 11) % 13 - 6) * 80;
        }

        // Update positions, track old rects for dirty
        let mut dirty_x0 = W as i32;
        let mut dirty_y0 = H as i32;
        let mut dirty_x1: i32 = 0;
        let mut dirty_y1: i32 = 0;

        for i in 0..3 {
            // Old position contributes to dirty
            if let Some(style) = world.get::<Style>(img_entities[i]) {
                let ox = style.layout.left.unwrap_or(0);
                let oy = style.layout.top.unwrap_or(0);
                dirty_x0 = dirty_x0.min(ox);
                dirty_y0 = dirty_y0.min(oy);
                dirty_x1 = dirty_x1.max(ox + iw as i32);
                dirty_y1 = dirty_y1.max(oy + ih as i32);
            }

            let bx = bodies[i].x / 256 - iw as i32 / 2;
            let by = bodies[i].y / 256 - ih as i32 / 2;

            // New position contributes to dirty
            dirty_x0 = dirty_x0.min(bx);
            dirty_y0 = dirty_y0.min(by);
            dirty_x1 = dirty_x1.max(bx + iw as i32);
            dirty_y1 = dirty_y1.max(by + ih as i32);

            if let Some(style) = world.get_mut::<Style>(img_entities[i]) {
                style.layout.left = Some(bx);
                style.layout.top = Some(by);
            }
        }

        // Clamp dirty rect
        dirty_x0 = dirty_x0.max(0);
        dirty_y0 = dirty_y0.max(0);
        dirty_x1 = dirty_x1.min(W as i32);
        dirty_y1 = dirty_y1.min(H as i32);
        let dw = (dirty_x1 - dirty_x0) as u16;
        let dh = (dirty_y1 - dirty_y0) as u16;

        if dw > 0 && dh > 0 {
            let dr = mirui::types::Rect { x: dirty_x0, y: dirty_y0, w: dw, h: dh };

            // Render only dirty region
            let mut renderer = SwRenderer::new(&mut rgba, W as u32, H as u32);
            render_system::render_region(&world, root, W, H, 1, &dr, &mut renderer);

            // Push only dirty region
            lcd.push_region(&rgba, W, dirty_x0 as u16, dirty_y0 as u16, dw, dh);

            // FPS overlay — draw directly, push its own small region
            draw_fps(&mut rgba, W, fps_display);
            lcd.push_region(&rgba, W, W - 50, 0, 50, 10);
        }
    }
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
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
