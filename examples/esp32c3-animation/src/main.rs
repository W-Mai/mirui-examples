#![no_std]
#![no_main]

extern crate alloc;

use esp_alloc as _;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::dma_buffers;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::spi::Mode as SpiMode;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::Rate;

// Two demo families share this binary. `demo-shapes` / `demo-butterfly`
// drive the ST7735 directly from their own loop; everything else plugs
// into mirui::App. Types that only appear on the App path (ECS World,
// App, FPS resources, perf plugin) are cfg-gated on "not shapes/butterfly".

#[cfg(feature = "app-demo")]
use mirui::prelude::{App, World};
use mirui::surface::framebuf::FramebufSurface;
#[cfg(feature = "app-demo")]
use mirui::surface::Surface;
use mirui::types::Rect;

mod board;
#[cfg(feature = "app-demo")]
mod esp_plugins;
#[cfg(feature = "esp-test-offscreen")]
mod esp_test_offscreen;

use board::{H, St7735, W};

esp_bootloader_esp_idf::esp_app_desc!();

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
// established polyfill (same shape mirui::perf uses).
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

#[cfg(feature = "app-demo")]
pub struct FrameCounter(pub u32);

#[cfg(feature = "app-demo")]
#[mirui::system]
fn frame_counter_system(world: &mut World) {
    let _n = if let Some(fc) = world.resource_mut::<FrameCounter>() {
        fc.0 = fc.0.wrapping_add(1);
        fc.0
    } else {
        0
    };
    #[cfg(feature = "perf-plan-probe")]
    if _n.is_multiple_of(30) {
        if let Some(p) = world.resource::<mirui::widget::render_system::LastDirtyRegions>() {
            for (i, r) in p.0.rects.iter().enumerate() {
                esp_println::println!(
                    "[plan] f={} rect[{}] {}x{}@({},{})",
                    _n,
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
                    _n,
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
}

#[esp_hal::main]
fn main() -> ! {
    esp_alloc::heap_allocator!(size: 200 * 1024);

    // Diagnostic harness short-circuits before peripherals — it doesn't
    // need SPI/LCD, just heap + UART (esp_println uses the boot UART
    // directly).
    #[cfg(feature = "esp-test-offscreen")]
    {
        esp_test_offscreen::run();
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
    // Drop to 600 / 50 / 30 when capturing visual snapshots.
    const CAPTURE_EVERY: u32 = 1_000_000;

    let flush_cb = move |buf: &[u8], area: &Rect| {
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

        // SystimerClockPlugin populates MonoClock; gallery demos read it
        // during build_widgets to seed time-driven animations.
        app.add_plugin(esp_plugins::SystimerClockPlugin);
        app.add_system(frame_counter_system::system());
        app.world.insert_resource(FrameCounter(0));

        let logical_w = app.backend.display_info().width;
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
            let parent = mirui::widget::builder::WidgetBuilder::new(&mut app.world).id();
            three_body::build_widgets(&mut app.world, parent, logical_w, logical_h, n_bodies, eq);
            app.set_root(parent);
        }

        #[cfg(feature = "demo-subpixel")]
        {
            use mirui::gallery::demos::subpixel;
            app.add_system(subpixel::bar_move_system::system());
            let parent = mirui::widget::builder::WidgetBuilder::new(&mut app.world).id();
            subpixel::build_widgets(&mut app.world, parent, logical_w, logical_h);
            app.set_root(parent);
        }

        #[cfg(feature = "demo-particles")]
        {
            use mirui::gallery::demos::particles;
            app.add_system(particles::particle_system::system());
            app.add_system(particles::pulse_ring_system::system());
            app.add_system(particles::bar_system::system());
            let parent = mirui::widget::builder::WidgetBuilder::new(&mut app.world).id();
            particles::build_widgets(&mut app.world, parent, logical_w, logical_h);
            app.set_root(parent);
        }

        #[cfg(feature = "demo-flipcard")]
        {
            use mirui::gallery::demos::flip_card;
            app.add_system(flip_card::flip_system::system());
            let parent = mirui::widget::builder::WidgetBuilder::new(&mut app.world).id();
            flip_card::build_widgets(&mut app.world, parent, logical_w, logical_h);
            app.set_root(parent);
        }

        #[cfg(feature = "demo-coverflow")]
        {
            use mirui::gallery::demos::cover_flow;
            app.add_system(cover_flow::layout_system::system());
            let parent = mirui::widget::builder::WidgetBuilder::new(&mut app.world).id();
            cover_flow::build_widgets(&mut app.world, parent, logical_w, logical_h);
            app.set_root(parent);
        }

        // demo-gesture: ESP's old hand-rolled Slider/Switch handler is
        // superseded by internal gesture in v0.27.2+. Routes to slider_switch.
        #[cfg(feature = "demo-gesture")]
        {
            use mirui::gallery::demos::slider_switch;
            let parent = mirui::widget::builder::WidgetBuilder::new(&mut app.world).id();
            slider_switch::build_widgets(&mut app.world, parent);
            app.set_root(parent);
        }

        #[cfg(feature = "demo-widgets")]
        {
            use mirui::gallery::demos::widgets;
            app.add_plugin(mirui::plugins::InputFeedbackPlugin::default());
            app.with_offscreen_pool_budget(32 * 1024);
            app.add_system(mirui::event::sim::sim_timeline_system::system());
            app.add_system(widgets::slider_to_progress_system::system());
            app.world.insert_resource(widgets::dark_with_accent());
            let cycle_e = widgets::Cycle::install(&mut app.world);
            app.world.insert(cycle_e, widgets::ThemeCycleIndex(0));
            let parent = mirui::widget::builder::WidgetBuilder::new(&mut app.world).id();
            widgets::build_widgets(&mut app.world, parent, logical_w, logical_h);
            app.set_root(parent);
        }

        #[cfg(feature = "demo-effects")]
        {
            use mirui::ecs;
            use mirui::gallery::demos::effect_glass;
            app.add_system(ecs::System::new(
                "glass_x",
                ecs::run_order::ANIMATION,
                effect_glass::GlassX::system(),
            ));
            app.add_system(ecs::System::new(
                "gauss_radius",
                ecs::run_order::ANIMATION,
                effect_glass::GaussRadius::system(),
            ));
            app.with_offscreen_pool_budget(8 * 1024);
            let parent = mirui::widget::builder::WidgetBuilder::new(&mut app.world).id();
            effect_glass::build_widgets(&mut app.world, parent, logical_w, logical_h);
            app.set_root(parent);
        }

        #[cfg(feature = "demo-shapes")]
        {
            use mirui::gallery::demos::shapes;
            app.with_widget(shapes::shapes_view());
            app.add_system(shapes::shapes_anim_system::system());
            let parent = mirui::widget::builder::WidgetBuilder::new(&mut app.world).id();
            shapes::build_widgets(&mut app.world, parent, logical_w, logical_h);
            app.set_root(parent);
        }

        #[cfg(feature = "demo-butterfly")]
        {
            use mirui::gallery::demos::butterfly;
            app.with_widget(butterfly::butterfly_view());
            app.add_system(butterfly::butterfly_anim_system::system());
            let parent = mirui::widget::builder::WidgetBuilder::new(&mut app.world).id();
            butterfly::build_widgets(&mut app.world, parent, logical_w, logical_h);
            app.set_root(parent);
        }

        #[cfg(feature = "perf-fps")]
        {
            let perf_report = mirui::plugins::PerfReportPlugin::new(100)
                .with_sink(esp_plugins::esp_span_report_sink);
            #[cfg(feature = "perf-trace")]
            let perf_report = perf_report.with_perfetto_line_sink(esp_plugins::esp_perfetto_box());
            // Budget +20% above measured baseline.
            let budget = mirui::plugins::BudgetReportPlugin::new(100)
                .with_avg_budget(17_000_000)
                .with_p99_budget(22_000_000)
                .with_sink(esp_plugins::esp_budget_sink);
            app.add_plugin(
                mirui::plugins::FpsSummaryPlugin::new(100).with_sink(esp_plugins::esp_perf_sink),
            )
            .add_plugin(perf_report)
            .add_plugin(budget);
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
