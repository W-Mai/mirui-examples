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

// Two demo families share this binary. `demo-shapes` / `demo-butterfly`
// drive the ST7735 directly from their own loop; everything else plugs
// into mirui::App. Types that only appear on the App path (ECS World,
// App, FPS resources, perf plugin) are cfg-gated on "not shapes/butterfly".

#[cfg(feature = "app-demo")]
use mirui::app::App;
#[cfg(feature = "app-demo")]
use mirui::ecs::World;
use mirui::surface::framebuf::FramebufSurface;
use mirui::types::Rect;

mod board;
#[cfg(feature = "app-demo")]
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
#[cfg(feature = "demo-gesture")]
mod demo_gesture;
#[cfg(feature = "demo-widgets")]
mod demo_widgets;


use board::{systimer_now, St7735, H, W};

esp_bootloader_esp_idf::esp_app_desc!();

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
        out[o + 2] = B64_ALPHABET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize];
        out[o + 3] = B64_ALPHABET[(b2 & 0x3f) as usize];
        i += 3;
        o += 4;
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
        out[o + 2] = B64_ALPHABET[((b1 & 0x0f) << 2) as usize];
        out[o + 3] = b'=';
        o += 4;
    }
    o
}

// Latest FPS value (one-second window). Single-writer on single-core
// RV32 with no reentrancy, so `static mut` is safe here.
#[cfg(feature = "fps-overlay")]
static mut FPS_DISPLAY: u32 = 0;

// Cumulative flush time within each 1-second report window; only the
// App-based demos maintain this, shapes/butterfly don't need it.
#[cfg(feature = "app-demo")]
static mut FLUSH_ACC: u32 = 0;

#[cfg(feature = "app-demo")]
pub struct FrameCounter(pub u32);

#[cfg(feature = "app-demo")]
struct FpsState {
    count: u32,
    last_tick: u32,
    display: u32,
}

#[cfg(feature = "app-demo")]
#[mirui::system]
fn frame_counter_system(world: &mut World) {
    if let Some(fc) = world.resource_mut::<FrameCounter>() {
        fc.0 = fc.0.wrapping_add(1);
    }
}

#[cfg(feature = "app-demo")]
#[mirui::system]
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
        #[cfg(feature = "fps-overlay")]
        unsafe {
            FPS_DISPLAY = fps.display;
        }
        let s = mirui::draw::quad_perf::take();
        let fill_us = s.fill_ticks / 160;
        let blit_us = s.blit_ticks / 160;
        esp_println::println!(
            "[quad] fill: {} calls {} us ({} px scan / {} draw / inset_hit {} / slow_hit {})",
            s.fill_count,
            fill_us,
            s.fill_scanned,
            s.fill_drawn,
            s.fill_inset_hit,
            s.fill_slow_hit
        );
        esp_println::println!(
            "[quad] blit: {} calls {} us ({} px scan / {} draw)",
            s.blit_count,
            blit_us,
            s.blit_scanned,
            s.blit_drawn
        );
    }
}

#[esp_hal::main]
fn main() -> ! {
    esp_alloc::heap_allocator!(size: 200 * 1024);
    let peripherals = esp_hal::init(esp_hal::Config::default());

    #[cfg(feature = "app-demo")]
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

    // Capture: every N flushes, emit the entire framebuffer as
    // base64-encoded raw pixel bytes between [CAP_BEGIN]/[CAP_END]
    // markers so a host script can dump it as a PNG. 32 KB at 115200
    // baud takes ~4 s to drain; renderer stalls for that window. Set
    // wide enough so most of the frame budget is still rendering.
    let mut capture_counter: u32 = 0;
    const CAPTURE_EVERY: u32 = 600;

    let flush_cb = move |buf: &[u8], area: &Rect| {
        #[cfg(feature = "app-demo")]
        let ft0 = systimer_now();
        let (x0, y0, x1, y1) = area.pixel_bounds();
        let x = x0.max(0) as u16;
        let y = y0.max(0) as u16;
        let w = ((x1.max(0) as u16).min(W)).saturating_sub(x);
        let h = ((y1.max(0) as u16).min(H)).saturating_sub(y);
        if w > 0 && h > 0 {
            lcd.push_region_raw(buf, W, x, y, w, h);
        }

        capture_counter = capture_counter.wrapping_add(1);
        if capture_counter % CAPTURE_EVERY == 0 {
            esp_println::println!(
                "[CAP_BEGIN] w={} h={} fmt=RGB565Swapped len={}",
                W,
                H,
                buf.len()
            );
            // Chunked base64 to keep individual lines under typical UART
            // ring buffer sizes; emit ~64 raw bytes per line.
            const CHUNK: usize = 48;
            let mut idx = 0;
            while idx < buf.len() {
                let end = (idx + CHUNK).min(buf.len());
                let mut out = [0u8; 64 + 4];
                let n = b64_encode(&buf[idx..end], &mut out);
                let s = core::str::from_utf8(&out[..n]).unwrap_or("");
                esp_println::println!("[CAP] {}", s);
                idx = end;
            }
            esp_println::println!("[CAP_END]");
        }
        #[cfg(feature = "fps-overlay")]
        {
            let fps = unsafe { FPS_DISPLAY };
            board::draw_fps_lcd(&mut lcd, fps);
        }
        #[cfg(feature = "app-demo")]
        {
            let ft1 = systimer_now();
            unsafe {
                FLUSH_ACC = FLUSH_ACC.wrapping_add(ft1.wrapping_sub(ft0));
            }
        }
    };

    // -----------------------------------------------------------------
    // Direct-driver demos (`demo-shapes` / `demo-butterfly`): own their
    // frame loop, don't touch mirui::App. They take a FramebufSurface
    // and step per frame; FPS reporting is done on the loop side, not
    // through the ECS.
    // -----------------------------------------------------------------
    #[cfg(not(feature = "app-demo"))]
    {
        let mut fb = FramebufSurface::with_format(
            W,
            H,
            mirui::draw::texture::ColorFormat::RGB565Swapped,
            flush_cb,
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
                #[cfg(feature = "fps-overlay")]
                unsafe {
                    FPS_DISPLAY = frame_count;
                }
                frame_count = 0;
                last_report = now;
            }
        }
    }

    // -----------------------------------------------------------------
    // mirui::App-based demos: build the backend with HiDPI options if
    // requested, register the demo's setup, add perf/clock plugins,
    // hand off to `app.run()`.
    // -----------------------------------------------------------------
    #[cfg(feature = "app-demo")]
    {
        #[cfg(feature = "demo-hidpi-downscale")]
        let backend = FramebufSurface::with_scale_and_format(
            W,
            H,
            mirui::types::Fixed::ONE / mirui::types::Fixed::from(2),
            mirui::draw::texture::ColorFormat::RGB565Swapped,
            flush_cb,
        );
        #[cfg(feature = "demo-hidpi-upscale")]
        let backend = FramebufSurface::with_scale_and_format(
            W,
            H,
            mirui::types::Fixed::from(2),
            mirui::draw::texture::ColorFormat::RGB565Swapped,
            flush_cb,
        );
        #[cfg(not(any(feature = "demo-hidpi-downscale", feature = "demo-hidpi-upscale")))]
        let backend = FramebufSurface::with_format(
            W,
            H,
            mirui::draw::texture::ColorFormat::RGB565Swapped,
            flush_cb,
        );

        let mut app = App::new(backend);
        app.with_default_widgets().with_default_systems();

        app.add_system(frame_counter_system::system());
        app.add_system(fps_system::system());
        app.world.insert_resource(FrameCounter(0));
        app.world.insert_resource(FpsState {
            count: 0,
            last_tick: systimer_now(),
            display: 0,
        });

        // HiDPI downscale doubles the logical viewport (128 → 256),
        // upscale halves it (128 → 64); both shift the demo tuning.
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

        #[cfg(feature = "demo-gesture")]
        demo_gesture::setup(&mut app);

        #[cfg(feature = "demo-widgets")]
        demo_widgets::setup(&mut app);

        app.add_plugin(esp_plugins::SystimerClockPlugin)
            .add_plugin(esp_plugins::EspPerfSummaryPlugin::default());

        app.run();
        unreachable!();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    esp_println::println!("[PANIC] {}", info);
    loop {
        core::hint::spin_loop();
    }
}
