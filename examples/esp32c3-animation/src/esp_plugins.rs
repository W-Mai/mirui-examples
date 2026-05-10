use alloc::boxed::Box;

use mirui::app::{App, RendererFactory};
use mirui::backend::Backend;
use mirui::ecs::World;
use mirui::plugin::Plugin;

use crate::board::systimer_now;

/// CPU frequency in MHz. `systimer_now()` reads mcycle, which ticks at CPU
/// clock rate, so `ticks / CPU_MHZ = µs` and `ticks * 1000 / CPU_MHZ = ns`.
const CPU_MHZ: u64 = 160;

/// Installs App::clock backed by the ESP32-C3 mcycle counter. Returns
/// nanoseconds elapsed since the plugin was built.
#[derive(Default)]
pub struct SystimerClockPlugin;

impl<B, F> Plugin<B, F> for SystimerClockPlugin
where
    B: Backend,
    F: RendererFactory,
{
    fn build(&mut self, app: &mut App<B, F>) {
        let start_ticks = systimer_now() as u64;
        app.clock = Box::new(move || {
            let now = systimer_now() as u64;
            now.wrapping_sub(start_ticks).saturating_mul(1000) / CPU_MHZ
        });
    }
}

/// Prints average render time every N frames via esp-println. Uses the
/// nanoseconds supplied by whichever clock plugin is installed (or 0 if
/// none, in which case this plugin still reports frame count).
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
    B: Backend,
    F: RendererFactory,
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
