use mirui::app::{App, RendererFactory};
use mirui::ecs::MonoClock;
use mirui::plugin::Plugin;
use mirui::plugins::FpsSummary;
use mirui::surface::Surface;

// esp_hal::time::Instant wraps after >7 years (uses the full 52-bit
// systimer counter, not just the low 32 bits we read from CSR 0x7E2
// in board::systimer_now). The previous implementation read CSR
// systimer_low and treated u32 cycle wrap (every 26.8s @ 160 MHz) as
// a normal monotonic clock, which is why anything past the first
// 26.8s of runtime would jump backwards by ~26.8s and corrupt every
// downstream elapsed-ms calculation (sim_timeline cycle drift,
// animation tick clamp, gesture recognizer timing).
fn esp_clock_ns() -> u64 {
    let micros = esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_micros();
    micros.saturating_mul(1000)
}

#[derive(Default)]
pub struct SystimerClockPlugin;

impl<B, F> Plugin<B, F> for SystimerClockPlugin
where
    B: Surface,
    F: RendererFactory<B>,
{
    fn build(&mut self, app: &mut App<B, F>) {
        app.world.insert_resource(MonoClock::new(esp_clock_ns));
        // Wires the same clock into mirui::perf so `trace_span!` /
        // `#[trace_fn]` start recording on this MCU.
        mirui::perf::set_clock(esp_clock_ns);
    }
}

/// `FpsSummaryPlugin::with_sink` target for ESP. Pipes the same data
/// the std default sink writes to stderr through `esp_println`, plus
/// the per-name perf aggregation. Also publishes the latest fps
/// number to `crate::FPS_DISPLAY` for the LCD overlay.
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
        "[perf] {} frames | frame {}us ({} fps) = event {} + systems {} + layout {} + render {} + flush {} + seed {}",
        report.frames,
        report.avg_frame_ns / 1000,
        fps,
        report.avg_event_poll_ns / 1000,
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
    if !report.perf_events.is_empty() {
        let aggr = mirui::perf::aggregate(&report.perf_events);
        for stat in &aggr {
            esp_println::println!(
                "[perf] {:24} count {:>5}  avg {:>5}us  max {:>5}us",
                stat.name,
                stat.count,
                (stat.total_ns / stat.count as u64) / 1000,
                stat.max_ns / 1000,
            );
        }
    }
}
