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

use board::{draw_fps_lcd, systimer_now, St7735, H, W};

esp_bootloader_esp_idf::esp_app_desc!();

pub struct FrameCounter(pub u32);
struct FpsState { count: u32, last_tick: u32, display: u32 }

static mut FPS_DISPLAY: u32 = 0;

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
            .with_frequency(Rate::from_mhz(26))
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

    let backend = FramebufBackend::new(W, H, move |buf: &[u8], area: &Rect| {
        let (x0, y0, x1, y1) = area.pixel_bounds();
        let x = x0.max(0) as u16;
        let y = y0.max(0) as u16;
        let w = ((x1.max(0) as u16).min(W)).saturating_sub(x);
        let h = ((y1.max(0) as u16).min(H)).saturating_sub(y);
        if w > 0 && h > 0 {
            lcd.push_region(buf, W, x, y, w, h);
        }
        let fps = unsafe { FPS_DISPLAY };
        draw_fps_lcd(&mut lcd, fps);
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

    app.render();
    loop {
        app.systems.run_all(&mut app.world);
        app.render_dirty();
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
