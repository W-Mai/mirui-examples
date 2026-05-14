use mirui::ecs::MonoClock;
use mirui::app::{App, RendererFactory};
use mirui::ecs::World;
use mirui::plugin::Plugin;
use mirui::surface::Surface;

use crate::board::systimer_now;

const CPU_MHZ: u64 = 160;

static mut CLOCK_START: u64 = 0;

fn esp_clock_ns() -> u64 {
    let now = systimer_now() as u64;
    unsafe { now.wrapping_sub(CLOCK_START).saturating_mul(1000) / CPU_MHZ }
}

#[derive(Default)]
pub struct SystimerClockPlugin;

impl<B, F> Plugin<B, F> for SystimerClockPlugin
where
    B: Surface,
    F: RendererFactory<B>,
{
    fn build(&mut self, app: &mut App<B, F>) {
        unsafe {
            CLOCK_START = systimer_now() as u64;
        }
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
