#![no_std]
#![no_main]

extern crate alloc;

use esp_alloc as _;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::dma_buffers;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::spi::Mode as SpiMode;
use esp_hal::time::Rate;

use mirui::app::App;
use mirui::backend::framebuf::FramebufBackend;
use mirui::ecs::World;
use mirui::types::Rect;

mod board;
mod esp_plugins;
#[cfg(feature = "demo-threebody")]
mod demo_threebody;
#[cfg(feature = "demo-subpixel")]
mod demo_subpixel;
#[cfg(feature = "demo-particles")]
mod demo_particles;
#[cfg(feature = "demo-shapes")]
mod demo_shapes;
#[cfg(feature = "demo-butterfly")]
mod demo_butterfly;
#[cfg(feature = "demo-flipcard")]
mod demo_flipcard;
#[cfg(feature = "demo-coverflow")]
mod demo_coverflow;

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
    let Some(fps) = world.resource_mut::<FpsState>() else {
        return;
    };
    fps.count += 1;
    if now.wrapping_sub(fps.last_tick) >= 160_000_000 {
        fps.display = fps.count;
        fps.count = 0;
        fps.last_tick = now;
        unsafe {
            FPS_DISPLAY = fps.display;
        }
        let (fill_ns, fill_cnt, blit_ns, blit_cnt) = mirui::draw::quad_perf::take();
        let fill_us = fill_ns / 160;
        let blit_us = blit_ns / 160;
        esp_println::println!(
            "[quad] fill: {} calls {} us  blit: {} calls {} us",
            fill_cnt,
            fill_us,
            blit_cnt,
            blit_us
        );
    }
}

#[cfg(any(feature = "demo-shapes", feature = "demo-butterfly"))]
#[esp_hal::main]
fn main() -> ! {
    esp_alloc::heap_allocator!(size: 200 * 1024);
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let (rx_buf, rx_desc, tx_buf, tx_desc) = dma_buffers!(32000);
    let dma_rx_buf = DmaRxBuf::new(rx_desc, rx_buf).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_desc, tx_buf).unwrap();

    let spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(32))
            .with_mode(SpiMode::_0),
    )
    .unwrap()
    .with_sck(peripherals.GPIO5)
    .with_mosi(peripherals.GPIO4)
    .with_dma(peripherals.DMA_CH0)
    .with_buffers(dma_rx_buf, dma_tx_buf);

    let cs = Output::new(peripherals.GPIO6, Level::High, OutputConfig::default());
    let dc = Output::new(peripherals.GPIO7, Level::Low, OutputConfig::default());
    let mut rst = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());
    let mut bl = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());

    let mut lcd = St7735 { spi, dc, cs };
    lcd.init(&mut rst);
    bl.set_high();

    let mut fb = FramebufBackend::with_format(
        W,
        H,
        mirui::draw::texture::ColorFormat::RGB565Swapped,
        move |buf: &[u8], area: &Rect| {
            let (x0, y0, x1, y1) = area.pixel_bounds();
            let x = x0.max(0) as u16;
            let y = y0.max(0) as u16;
            let w = ((x1.max(0) as u16).min(W)).saturating_sub(x);
            let h = ((y1.max(0) as u16).min(H)).saturating_sub(y);
            if w > 0 && h > 0 {
                lcd.push_region_raw(buf, W, x, y, w, h);
            }
        },
    );

    #[cfg(feature = "demo-shapes")]
    let (mut demo, tag) = (demo_shapes::ShapesDemo::new(), "shapes");
    #[cfg(feature = "demo-butterfly")]
    let (mut demo, tag) = (demo_butterfly::ButterflyDemo::new(), "butterfly");

    let mut last_report = systimer_now();
    let mut frame_count: u32 = 0;
    loop {
        demo.step(&mut fb);
        frame_count += 1;
        let now = systimer_now();
        if now.wrapping_sub(last_report) >= 160_000_000 {
            esp_println::println!("[{}] fps={}", tag, frame_count);
            frame_count = 0;
            last_report = now;
        }
    }
}

#[cfg(not(any(feature = "demo-shapes", feature = "demo-butterfly")))]
#[esp_hal::main]
fn main() -> ! {
    esp_alloc::heap_allocator!(size: 200 * 1024);
    let peripherals = esp_hal::init(esp_hal::Config::default());

    unsafe {
        mirui::draw::quad_perf::CLOCK = || board::systimer_now() as u64;
    }

    let (rx_buf, rx_desc, tx_buf, tx_desc) = dma_buffers!(32000);
    let dma_rx_buf = DmaRxBuf::new(rx_desc, rx_buf).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_desc, tx_buf).unwrap();

    let spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(32))
            .with_mode(SpiMode::_0),
    )
    .unwrap()
    .with_sck(peripherals.GPIO5)
    .with_mosi(peripherals.GPIO4)
    .with_dma(peripherals.DMA_CH0)
    .with_buffers(dma_rx_buf, dma_tx_buf);

    let cs = Output::new(peripherals.GPIO6, Level::High, OutputConfig::default());
    let dc = Output::new(peripherals.GPIO7, Level::Low, OutputConfig::default());
    let mut rst = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());
    let mut bl = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());

    let mut lcd = St7735 { spi, dc, cs };
    lcd.init(&mut rst);
    bl.set_high();

    let flush_cb = move |buf: &[u8], area: &Rect| {
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
        unsafe {
            FLUSH_ACC = FLUSH_ACC.wrapping_add(ft1.wrapping_sub(ft0));
        }
    };

    #[cfg(feature = "demo-hidpi-downscale")]
    let backend = FramebufBackend::with_scale_and_format(
        W,
        H,
        mirui::types::Fixed::ONE / mirui::types::Fixed::from(2),
        mirui::draw::texture::ColorFormat::RGB565Swapped,
        flush_cb,
    );
    #[cfg(feature = "demo-hidpi-upscale")]
    let backend = FramebufBackend::with_scale_and_format(
        W,
        H,
        mirui::types::Fixed::from(2),
        mirui::draw::texture::ColorFormat::RGB565Swapped,
        flush_cb,
    );
    #[cfg(not(any(feature = "demo-hidpi-downscale", feature = "demo-hidpi-upscale")))]
    let backend = FramebufBackend::with_format(
        W,
        H,
        mirui::draw::texture::ColorFormat::RGB565Swapped,
        flush_cb,
    );

    let mut app = App::new(backend);

    // Shared systems
    app.add_system(frame_counter_system);
    app.add_system(fps_system);
    app.world.insert_resource(FrameCounter(0));
    app.world.insert_resource(FpsState { count: 0, last_tick: systimer_now(), display: 0 });

    // HiDPI downscale doubles the logical viewport (128 → 256),
    // HiDPI upscale halves it (128 → 64); both shift the demo tuning.
    #[cfg(all(
        feature = "demo-threebody",
        not(any(feature = "demo-hidpi-downscale", feature = "demo-hidpi-upscale"))
    ))]
    demo_threebody::setup(&mut app, 3, mirui::types::Fixed::from_int(30));
    #[cfg(all(feature = "demo-threebody", feature = "demo-hidpi-downscale"))]
    demo_threebody::setup(&mut app, 6, mirui::types::Fixed::from_int(60));
    #[cfg(all(feature = "demo-threebody", feature = "demo-hidpi-upscale"))]
    demo_threebody::setup(&mut app, 3, mirui::types::Fixed::from_int(15));

    #[cfg(feature = "demo-subpixel")]
    demo_subpixel::setup(&mut app);

    #[cfg(feature = "demo-particles")]
    demo_particles::setup(&mut app);

    #[cfg(feature = "demo-flipcard")]
    demo_flipcard::setup(&mut app);

    #[cfg(feature = "demo-coverflow")]
    demo_coverflow::setup(&mut app);

    app.add_plugin(esp_plugins::SystimerClockPlugin)
        .add_plugin(esp_plugins::EspPerfSummaryPlugin::default());

    app.run();
    unreachable!();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    esp_println::println!("[PANIC] {}", info);
    loop {
        core::hint::spin_loop();
    }
}
