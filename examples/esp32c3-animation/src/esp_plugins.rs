use mirui::app::{App, RendererFactory};
use mirui::ecs::MonoClock;
use mirui::plugin::Plugin;
#[cfg(feature = "perf-fps")]
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
#[cfg(feature = "perf-fps")]
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
#[cfg(feature = "perf-fps")]
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
    for s in &report.systems {
        if s.call_count == 0 {
            continue;
        }
        esp_println::println!(
            "[sys ] {:24} count {:>5}  avg {:>5}us  last {:>5}us",
            s.name,
            s.call_count,
            s.avg_us,
            s.last_us,
        );
    }
}

/// `[trace]` prefix is what `tools/esp-trace.py` greps for.
#[cfg(feature = "perf-trace")]
pub fn esp_perfetto_box() -> mirui::plugins::PerfettoLineSink {
    alloc::boxed::Box::new(|batch: &str| {
        // One esp_println per frame instead of per event — each call
        // walks a critical section + USB Serial-JTAG FIFO flush.
        let mut out = alloc::string::String::with_capacity(batch.len() + 64);
        for line in batch.lines() {
            out.push_str("[trace] ");
            out.push_str(line);
            out.push('\n');
        }
        esp_println::print!("{}", out);
    })
}

/// `BudgetReportPlugin` sink: prints over esp_println and bumps the
/// LCD overlay counter (red `<n>!` line under the fps readout).
#[cfg(feature = "perf-fps")]
pub fn esp_budget_sink(v: mirui::plugins::BudgetViolation) {
    esp_println::println!(
        "[budget] avg {}us (budget {}us) p99 {}us (budget {}us) jitter {}us",
        v.avg_ns / 1000,
        v.budget_avg_ns / 1000,
        v.p99_ns / 1000,
        v.budget_p99_ns / 1000,
        v.jitter_ns / 1000,
    );
    #[cfg(feature = "fps-overlay")]
    crate::budget_violations_inc();
}
