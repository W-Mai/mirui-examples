use mirui::app::{App, RendererFactory};
use mirui::ecs::MonoClock;
use mirui::plugin::Plugin;
use mirui::plugins::FpsSummary;
use mirui::surface::Surface;

// esp_hal Instant uses the full 52-bit systimer; csr 0x7E2 wraps
// every 26.8s and was the source of the v0.10-v0.17 fps drift.
fn esp_clock_ns() -> u64 {
    let micros = esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_micros();
    micros.saturating_mul(1000)
}

/// Backs `MonoClock` and `crate::perf` with `esp_hal::time::Instant`.
///
/// **Inserts**
/// - resource: `MonoClock`
/// - global: calls `mirui::perf::set_clock` so `trace_span!` records
#[derive(Default)]
pub struct SystimerClockPlugin;

impl<B, F> Plugin<B, F> for SystimerClockPlugin
where
    B: Surface,
    F: RendererFactory<B>,
{
    fn build(&mut self, app: &mut App<B, F>) {
        app.world.insert_resource(MonoClock::new(esp_clock_ns));
        mirui::perf::set_clock(esp_clock_ns);
    }
}

/// `FpsSummaryPlugin` sink. Side effect: writes `crate::FPS_DISPLAY`
/// for the LCD overlay. Per-span detail and Chrome-trace JSON come
/// from `PerfReportPlugin` instead of being re-implemented here.
pub fn esp_perf_sink(report: FpsSummary<'_>) {
    let fps = if report.avg_frame_ns == 0 {
        0
    } else {
        1_000_000_000 / report.avg_frame_ns
    };
    #[cfg(feature = "fps-overlay")]
    unsafe {
        crate::FPS_DISPLAY = fps as u32;
    }
    esp_println::println!(
        "[perf] {} frames | frame {}us ({} fps) = input {} + systems {} + layout {} + render {} + flush {} + seed {}",
        report.frames,
        report.avg_frame_ns / 1000,
        fps,
        report.avg_input_ns / 1000,
        report.avg_systems_ns / 1000,
        report.avg_layout_ns / 1000,
        report.avg_render_ns / 1000,
        report.avg_flush_ns / 1000,
        report.avg_seed_prev_ns / 1000,
    );
    if let Some(s) = report.stats {
        esp_println::println!(
            "[perf] window={} min {}us max {}us p99 {}us jitter {}us",
            s.len(),
            s.min() / 1000,
            s.max() / 1000,
            s.p99() / 1000,
            s.jitter() / 1000,
        );
    }
}

/// `PerfReportPlugin` sink — per-span aggregates over `esp_println`.
pub fn esp_span_report_sink(report: &mirui::plugins::PerfReport) {
    for s in &report.stage_stats {
        if s.count == 0 {
            continue;
        }
        esp_println::println!(
            "[perf] {:24} count {:>5}  avg {:>5}us  max {:>5}us",
            s.name,
            s.count,
            (s.total_ns / s.count as u64) / 1000,
            s.max_ns / 1000,
        );
    }
}

/// Boxed sink for `PerfReportPlugin::with_perfetto_line_sink` — writes
/// each Chrome-trace JSON line through `esp_println` so the host-side
/// `tools/esp-trace.py` collector can read it off UART.
pub fn esp_perfetto_box() -> mirui::plugins::PerfettoLineSink {
    alloc::boxed::Box::new(|line: &str| {
        esp_println::println!("[trace] {}", line);
    })
}
