use mirui::prelude::*;
use mirui::ecs::MonoClock;
use mirui::app::{App, RendererFactory};
use mirui::plugin::Plugin;
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
    }
}

pub struct EspPerfSummaryPlugin {
    frames_per_summary: u32,
    frame_count: u32,
    render_ns_total: u64,
}

impl EspPerfSummaryPlugin {
    pub fn new(frames_per_summary: u32) -> Self {
        Self {
            frames_per_summary,
            frame_count: 0,
            render_ns_total: 0,
        }
    }
}

impl Default for EspPerfSummaryPlugin {
    fn default() -> Self {
        Self::new(100)
    }
}

impl<B, F> Plugin<B, F> for EspPerfSummaryPlugin
where
    B: Surface,
    F: RendererFactory<B>,
{
    fn build(&mut self, _app: &mut App<B, F>) {}

    fn post_render(&mut self, _world: &mut World, render_nanos: u64) {
        self.frame_count += 1;
        self.render_ns_total += render_nanos;
        if self.frame_count >= self.frames_per_summary {
            let avg_us = self.render_ns_total / self.frame_count as u64 / 1000;
            esp_println::println!(
                "[perf] {} frames, avg render {} us",
                self.frame_count,
                avg_us
            );
            self.frame_count = 0;
            self.render_ns_total = 0;
        }
    }
}
