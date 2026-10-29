use mirui::app::plugin::Plugin;
#[cfg(feature = "perf-fps")]
use mirui::app::plugins::FpsSummary;
use mirui::app::{App, RendererFactory};
use mirui::ecs::MonoClock;
use mirui::surface::Surface;

// esp_hal Instant uses the full 52-bit systimer; csr 0x7E2 wraps
// every 26.8s and was the source of the v0.10-v0.17 fps drift.
pub(super) fn esp_clock_ns() -> u64 {
    let micros = esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_micros();
    micros.saturating_mul(1000)
}

/// Backs mirui's process clock with `esp_hal::time::Instant`. Installs
/// once at boot; every downstream reader (MonoClock, mirui::info!,
/// trace_span!, App::clock_ns) shares the same nanoseconds afterwards.
///
/// **Inserts**
/// - global clock: `core::time::set_clock(esp_clock_ns)`
/// - resource: `MonoClock` reading the global clock
#[derive(Default)]
pub struct SystimerClockPlugin;

impl<B, F> Plugin<B, F> for SystimerClockPlugin
where
    B: Surface,
    F: RendererFactory<B>,
{
    fn build(&mut self, app: &mut App<B, F>) {
        mirui::core::time::set_clock(esp_clock_ns);
        app.world.insert_resource(MonoClock::from_time_source());
    }
}

/// `FpsSummaryPlugin` sink. Side effect: writes `crate::FPS_DISPLAY`
/// with Tick callback cadence for the LCD overlay, not display rate.
/// Per-span detail and Chrome-trace JSON come
/// from `PerfReportPlugin` instead of being re-implemented here.
#[cfg(feature = "perf-fps")]
pub fn esp_perf_sink(report: FpsSummary<'_>) {
    for operation in report
        .operations
        .iter()
        .filter(|operation| operation.count != 0)
    {
        let rate = report.operation_rate_hz(operation.operation);
        #[cfg(feature = "fps-overlay")]
        if operation.operation == mirui::ecs::FrameOperation::Tick {
            unsafe {
                crate::FPS_DISPLAY = rate.unwrap_or(0.0) as u32;
            }
        }
        esp_println::println!(
            "[perf] {:?} operations {} | wall {:?}ns | rate {:?}/s | CPU {:?}ns | display rate unavailable",
            operation.operation,
            operation.count,
            operation.wall_nanos(),
            rate,
            operation.thread_cpu_nanos(),
        );
        for phase in [
            mirui::ecs::FramePhase::Input,
            mirui::ecs::FramePhase::Systems,
            mirui::ecs::FramePhase::Layout,
            mirui::ecs::FramePhase::Render,
            mirui::ecs::FramePhase::Flush,
            mirui::ecs::FramePhase::SeedPrev,
        ] {
            if let Some(ns) = operation.phase_nanos(phase) {
                esp_println::println!(
                    "[phase] {:?} {}ns samples={}",
                    phase,
                    ns,
                    operation.phase_samples(phase)
                );
            }
        }
        if let Some(stats) = report
            .stats
            .and_then(|stats| stats.summary(operation.operation, operation.latest_phases))
        {
            esp_println::println!(
                "[perf] phase budget count={} avg={}ns min={}ns max={}ns p99={}ns jitter={}ns",
                stats.count,
                stats.avg_nanos,
                stats.min_nanos,
                stats.max_nanos,
                stats.p99_nanos,
                stats.jitter_nanos(),
            );
        }
    }
}

/// `PerfReportPlugin` sink — per-span aggregates over `esp_println`.
#[cfg(feature = "perf-trace")]
pub fn esp_span_report_sink(report: &mirui::app::plugins::PerfReport) {
    for s in report.stage_stats {
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
    for s in report.systems {
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
pub fn esp_perfetto_box() -> mirui::app::plugins::PerfettoLineSink {
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
pub fn esp_budget_sink(v: mirui::app::plugins::BudgetViolation) {
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
