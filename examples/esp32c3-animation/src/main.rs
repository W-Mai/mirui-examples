#![no_std]
#![no_main]

extern crate alloc;

use esp_alloc as _;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::spi::Mode as SpiMode;
use esp_hal::time::Rate;

use mirui::app::App;
use mirui::backend::framebuf::FramebufBackend;
use mirui::ecs::World;
use mirui::types::Rect;

mod board;
#[cfg(feature = "demo-threebody")]
mod demo_threebody;
#[cfg(feature = "demo-subpixel")]
mod demo_subpixel;
#[cfg(feature = "demo-particles")]
mod demo_particles;

use board::{draw_fps_lcd, systimer_now, St7735, H, W};

esp_bootloader_esp_idf::esp_app_desc!();

pub struct FrameCounter(pub u32);
struct FpsState { count: u32, last_tick: u32, display: u32 }

static mut FPS_DISPLAY: u32 = 0;
static mut FLUSH_ACC: u32 = 0;

fn frame_counter_system(world: &mut World) {
    if let Some(fc) = world.resource_mut::<FrameCounter>() {
        fc.0 = fc.0.wrapping_add(1);
    }
}

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

#[esp_hal::main]
fn main() -> ! {
    esp_alloc::heap_allocator!(size: 200 * 1024);
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(32))
            .with_mode(SpiMode::_0),
    )
    .unwrap()
    .with_sck(peripherals.GPIO5)
    .with_mosi(peripherals.GPIO4);

    let cs = Output::new(peripherals.GPIO6, Level::High, OutputConfig::default());
    let dc = Output::new(peripherals.GPIO7, Level::Low, OutputConfig::default());
    let mut rst = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());
    let mut bl = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());

    let mut lcd = St7735 { spi, dc, cs };
    lcd.init(&mut rst);
    bl.set_high();

    let backend = FramebufBackend::with_format(W, H, mirui::draw::texture::ColorFormat::RGB565Swapped, move |buf: &[u8], area: &Rect| {
        let ft0 = systimer_now();
        let (x0, y0, x1, y1) = area.pixel_bounds();
        let x = x0.max(0) as u16;
        let y = y0.max(0) as u16;
        let w = ((x1.max(0) as u16).min(W)).saturating_sub(x);
        let h = ((y1.max(0) as u16).min(H)).saturating_sub(y);
        if w > 0 && h > 0 {
            lcd.push_region_raw(buf, W, x, y, w, h);
        }
        let fps = unsafe { FPS_DISPLAY };
        draw_fps_lcd(&mut lcd, fps);
        let ft1 = systimer_now();
        unsafe { FLUSH_ACC = FLUSH_ACC.wrapping_add(ft1.wrapping_sub(ft0)); }
    });

    let mut app = App::new(backend);

    // Shared systems
    app.add_system(frame_counter_system);
    app.add_system(fps_system);
    app.world.insert_resource(FrameCounter(0));
    app.world.insert_resource(FpsState { count: 0, last_tick: systimer_now(), display: 0 });

    // Demo-specific setup
    #[cfg(feature = "demo-threebody")]
    demo_threebody::setup(&mut app);

    #[cfg(feature = "demo-subpixel")]
    demo_subpixel::setup(&mut app);

    #[cfg(feature = "demo-particles")]
    demo_particles::setup(&mut app);

    app.perf = Some(mirui::draw::PerfCtx::new(|| systimer_now() as u64));

    app.render();
    let mut perf_acc: [u32; 3] = [0; 3]; // systems, render, total
    let mut perf_frames: u32 = 0;
    loop {
        let t0 = systimer_now();
        app.systems.run_all(&mut app.world);
        let t1 = systimer_now();
        app.render_dirty();
        let t2 = systimer_now();

        perf_acc[0] = perf_acc[0].wrapping_add(t1.wrapping_sub(t0));
        perf_acc[1] = perf_acc[1].wrapping_add(t2.wrapping_sub(t1));
        perf_acc[2] = perf_acc[2].wrapping_add(t2.wrapping_sub(t0));
        perf_frames += 1;
        if perf_frames >= 100 {
            let flush_us = unsafe { FLUSH_ACC } / 160 / 100;
            unsafe { FLUSH_ACC = 0; }
            let (fill_us, stroke_us, blit_us, label_us) = if let Some(p) = app.perf.as_mut() {
                let r = (
                    (p.fill / 160 / 100) as u32,
                    (p.stroke / 160 / 100) as u32,
                    (p.blit / 160 / 100) as u32,
                    (p.label / 160 / 100) as u32,
                );
                p.reset();
                r
            } else {
                (0, 0, 0, 0)
            };
            esp_println::println!(
                "[perf] sys={}us fill={}us stroke={}us blit={}us label={}us flush={}us total={}us",
                perf_acc[0] / 160 / 100,
                fill_us,
                stroke_us,
                blit_us,
                label_us,
                flush_us,
                perf_acc[2] / 160 / 100,
            );
            perf_acc = [0; 3];
            perf_frames = 0;
        }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
