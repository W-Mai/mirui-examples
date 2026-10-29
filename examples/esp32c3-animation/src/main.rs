#![no_std]
#![no_main]

extern crate alloc;

use core::mem::MaybeUninit;
use core::ptr::addr_of_mut;
use embedded_alloc::TlsfHeap;
#[cfg(not(feature = "esp-test-offscreen"))]
use esp_hal::{
    dma::{DmaRxBuf, DmaTxBuf},
    dma_buffers,
    gpio::{Level, Output, OutputConfig},
    spi::{
        Mode as SpiMode,
        master::{Config as SpiConfig, Spi},
    },
    time::Rate,
};

#[cfg(all(feature = "app-demo", not(feature = "esp-test-offscreen")))]
use mirui::prelude::App;
#[cfg(all(
    any(feature = "demo-threebody", feature = "demo-coverflow"),
    not(feature = "esp-test-offscreen")
))]
use mirui::surface::Surface;
#[cfg(not(feature = "esp-test-offscreen"))]
use mirui::surface::framebuf::FramebufSurface;
#[cfg(not(feature = "esp-test-offscreen"))]
use mirui::types::PhysicalRect;

#[cfg(not(feature = "esp-test-offscreen"))]
mod board;
#[cfg(all(feature = "app-demo", not(feature = "esp-test-offscreen")))]
mod esp_plugins;
#[cfg(feature = "esp-test-offscreen")]
mod esp_test_offscreen;

#[cfg(not(feature = "esp-test-offscreen"))]
use board::{H, St7735, W};

esp_bootloader_esp_idf::esp_app_desc!();

#[cfg(feature = "frame-capture")]
const B64_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[cfg(feature = "frame-capture")]
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

// Single-writer on single-core RV32, no reentrancy → static mut is sound.
#[cfg(feature = "fps-overlay")]
pub static mut FPS_DISPLAY: u32 = 0;

// RV32IMC has no A extension, so AtomicU32::fetch_add fails to link.
// One critical_section guard around a plain `Cell<u32>` is the
// established polyfill (same shape mirui::core::perf uses).
#[cfg(feature = "fps-overlay")]
pub static BUDGET_VIOLATIONS: critical_section::Mutex<core::cell::Cell<u32>> =
    critical_section::Mutex::new(core::cell::Cell::new(0));

#[cfg(feature = "fps-overlay")]
pub fn budget_violations_load() -> u32 {
    critical_section::with(|cs| BUDGET_VIOLATIONS.borrow(cs).get())
}

#[cfg(feature = "fps-overlay")]
pub fn budget_violations_inc() {
    critical_section::with(|cs| {
        let cell = BUDGET_VIOLATIONS.borrow(cs);
        cell.set(cell.get().saturating_add(1));
    });
}

#[cfg(all(feature = "app-demo", feature = "perf-plan-probe"))]
pub struct FrameCounter(pub u32);

#[cfg(all(feature = "app-demo", feature = "perf-plan-probe"))]
#[mirui::system]
fn frame_counter_system(
    counter: res!(mut FrameCounter),
    plan: res!(optional mirui::ui::render_system::LastDirtyRegions),
) {
    counter.0 = counter.0.wrapping_add(1);
    let n = counter.0;
    if n.is_multiple_of(30)
        && let Some(p) = plan
    {
        for (i, r) in p.0.rects.iter().enumerate() {
            esp_println::println!(
                "[plan] f={} rect[{}] {}x{}@({},{})",
                n,
                i,
                r.w.to_int(),
                r.h.to_int(),
                r.x.to_int(),
                r.y.to_int(),
            );
        }
        for (i, s) in p.0.shifts.iter().enumerate() {
            esp_println::println!(
                "[plan] f={} scr[{}] {}x{}@({},{}) dy={}",
                n,
                i,
                s.area.w.to_int(),
                s.area.h.to_int(),
                s.area.x.to_int(),
                s.area.y.to_int(),
                s.dy.to_int(),
            );
        }
    }
}

#[global_allocator]
static HEAP: TlsfHeap = TlsfHeap::empty();

const HEAP_SIZE: usize = 200 * 1024;
static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];

#[esp_hal::main]
fn main() -> ! {
    unsafe {
        // SAFETY: HEAP_MEM is a static buffer used only here, before any allocation.
        HEAP.init(addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE);
    }

    // Diagnostic harness short-circuits before peripherals — it doesn't
    // need SPI/LCD, just heap + UART (esp_println uses the boot UART
    // directly).
    #[cfg(feature = "esp-test-offscreen")]
    {
        esp_test_offscreen::run_frames(|message| esp_println::println!("{}", message));
        loop {
            core::hint::spin_loop();
        }
    }

    #[cfg(not(feature = "esp-test-offscreen"))]
    {
        run_normal()
    }
}

#[cfg(not(feature = "esp-test-offscreen"))]
fn run_normal() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());

    #[cfg(all(feature = "app-demo", feature = "perf-fps"))]
    unsafe {
        mirui::render::quad_perf::CLOCK = || board::systimer_now() as u64;
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

    let mut lcd = St7735 {
        spi,
        dc,
        cs,
        #[cfg(feature = "fps-overlay")]
        overlay_font: mirui::render::font::Font::default(),
    };
    lcd.init(&mut rst);
    bl.set_high();

    #[cfg(feature = "frame-capture")]
    let mut capture_counter: u32 = 0;

    let flush_cb = move |buf: &[u8], area: PhysicalRect| {
        let x = area.x();
        let y = area.y();
        let w = area.width();
        let h = area.height();
        if w > 0 && h > 0 {
            lcd.push_region_raw(buf, W, x, y, w, h);
        }

        #[cfg(feature = "frame-capture")]
        {
            capture_counter = capture_counter.wrapping_add(1);
            if capture_counter.is_multiple_of(1_000_000) {
                esp_println::println!(
                    "[CAP_BEGIN] w={} h={} fmt=RGB565Swapped len={}",
                    W,
                    H,
                    buf.len()
                );
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
        }
        #[cfg(feature = "fps-overlay")]
        {
            let fps = unsafe { FPS_DISPLAY };
            let violations = budget_violations_load();
            board::draw_perf_overlay(&mut lcd, fps, violations);
        }
    };

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
            mirui::render::texture::ColorFormat::RGB565Swapped,
            flush_cb,
        );
        #[cfg(feature = "demo-hidpi-upscale")]
        let backend = FramebufSurface::with_scale_and_format(
            W,
            H,
            mirui::types::Fixed::from(2),
            mirui::render::texture::ColorFormat::RGB565Swapped,
            flush_cb,
        );
        #[cfg(not(any(feature = "demo-hidpi-downscale", feature = "demo-hidpi-upscale")))]
        let backend = FramebufSurface::with_format(
            W,
            H,
            mirui::render::texture::ColorFormat::RGB565Swapped,
            flush_cb,
        );
        #[cfg(feature = "demo-threebody")]
        let backend = backend.with_redraw_strategy(mirui::surface::RedrawStrategy::BoundingBox);

        let mut app = App::new(backend);
        app.with_default_widgets().with_default_systems();

        // SystimerClockPlugin populates MonoClock; gallery demos read it
        // during build_widgets to seed time-driven animations.
        app.add_plugin(esp_plugins::SystimerClockPlugin);
        app.add_plugin(mirui::app::plugins::ImageResourcesPlugin::default());
        #[cfg(feature = "perf-plan-probe")]
        {
            app.add_system(frame_counter_system::system());
            app.add_resource(FrameCounter(0));
        }

        #[cfg(any(feature = "demo-threebody", feature = "demo-coverflow"))]
        let logical_w = app.backend.display_info().width;
        #[cfg(any(feature = "demo-threebody", feature = "demo-coverflow"))]
        let logical_h = app.backend.display_info().height;

        #[cfg(feature = "demo-threebody")]
        {
            use mirui::gallery::demos::three_body;
            #[cfg(not(any(feature = "demo-hidpi-downscale", feature = "demo-hidpi-upscale")))]
            let (n_bodies, eq) = (3, mirui::types::Fixed::from_int(30));
            #[cfg(feature = "demo-hidpi-downscale")]
            let (n_bodies, eq) = (6, mirui::types::Fixed::from_int(60));
            #[cfg(feature = "demo-hidpi-upscale")]
            let (n_bodies, eq) = (3, mirui::types::Fixed::from_int(15));
            app.add_system(three_body::physics_tick_system::system());
            app.add_system(three_body::kick_system::system());
            app.add_system(three_body::sync_layout_system::system());
            let parent = app.spawn_root().id();
            app.compose(
                parent,
                three_body::build_widgets(logical_w, logical_h, n_bodies, eq),
            );
        }

        #[cfg(feature = "demo-subpixel")]
        {
            use mirui::gallery::demos::subpixel;
            app.add_system(subpixel::bar_move_system::system());
            let parent = app.spawn_root().id();
            app.compose(parent, subpixel::build_widgets());
        }

        #[cfg(feature = "demo-particles")]
        {
            use mirui::gallery::demos::particles;
            app.add_system(particles::particle_bounds_system::system());
            app.add_system(particles::particle_system::system());
            app.add_system(particles::pulse_ring_system::system());
            app.add_system(particles::bar_system::system());
            let parent = app.spawn_root().id();
            app.compose(parent, particles::build_widgets());
        }

        #[cfg(feature = "demo-flipcard")]
        {
            use mirui::gallery::demos::flip_card;
            app.add_system(flip_card::flip_system::system());
            let parent = app.spawn_root().id();
            app.compose(parent, flip_card::build_widgets());
        }

        #[cfg(feature = "demo-coverflow")]
        {
            use mirui::gallery::demos::cover_flow;
            app.add_system(cover_flow::layout_system::system());
            let parent = app.spawn_root().id();
            app.compose(parent, cover_flow::build_widgets(logical_w, logical_h));
        }

        #[cfg(feature = "demo-life")]
        {
            use mirui::gallery::demos::life;
            let parent = app.spawn_root().id();
            life::setup_app(&mut app, parent);
        }

        #[cfg(feature = "demo-typography")]
        {
            use mirui::gallery::demos::typography_lab;
            let parent = app.spawn_root().id();
            typography_lab::setup_app(&mut app, parent);
        }

        #[cfg(feature = "demo-interaction")]
        {
            use mirui::gallery::demos::interaction_lab;
            let parent = app.spawn_root().id();
            interaction_lab::setup_app(&mut app, parent);
        }

        #[cfg(feature = "demo-widgets")]
        {
            use mirui::gallery::demos::widgets;
            app.add_plugin(mirui::app::plugins::InputFeedbackPlugin::default());
            app.with_offscreen_pool_budget(32 * 1024);
            let parent = app.spawn_root().id();
            widgets::install(&mut app, parent, true);
        }

        #[cfg(feature = "demo-kinetic")]
        {
            use mirui::gallery::demos::kinetic_console;
            let parent = app.spawn_root().id();
            kinetic_console::install(&mut app, parent, true);
        }

        #[cfg(feature = "demo-curve-text-compact")]
        {
            use mirui::gallery::demos::curve_text_compact;
            let parent = app.spawn_root().id();
            curve_text_compact::install(&mut app, parent);
        }

        #[cfg(feature = "demo-effects")]
        {
            use mirui::gallery::demos::effect_glass;
            app.add_system(effect_glass::GlassX::system());
            app.add_system(effect_glass::GaussRadius::system());
            app.with_offscreen_pool_budget(8 * 1024);
            let parent = app.spawn_root().id();
            app.compose(parent, effect_glass::build_widgets());
        }

        #[cfg(feature = "demo-shapes")]
        {
            use mirui::gallery::demos::shapes;
            app.with_widget(shapes::shapes_view());
            app.add_system(shapes::shapes_anim_system::system());
            let parent = app.spawn_root().id();
            app.compose(parent, shapes::build_widgets());
        }

        #[cfg(feature = "demo-butterfly")]
        {
            use mirui::gallery::demos::butterfly;
            app.with_widget(butterfly::butterfly_view());
            app.add_system(butterfly::butterfly_anim_system::system());
            let parent = app.spawn_root().id();
            app.compose(parent, butterfly::build_widgets());
        }

        #[cfg(feature = "perf-fps")]
        {
            #[cfg(feature = "perf-trace")]
            let perf_report = mirui::app::plugins::PerfReportPlugin::new(100)
                .with_sink(esp_plugins::esp_span_report_sink)
                .with_perfetto_line_sink(esp_plugins::esp_perfetto_box());
            // Budget +20% above measured baseline.
            let budget = mirui::app::plugins::BudgetReportPlugin::new(100)
                .with_avg_budget(17_000_000)
                .with_p99_budget(22_000_000)
                .with_sink(esp_plugins::esp_budget_sink);
            app.add_plugin(
                mirui::app::plugins::FpsSummaryPlugin::new(100)
                    .with_sink(esp_plugins::esp_perf_sink),
            )
            .add_plugin(budget);
            #[cfg(feature = "perf-trace")]
            app.add_plugin(perf_report);
        }

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
