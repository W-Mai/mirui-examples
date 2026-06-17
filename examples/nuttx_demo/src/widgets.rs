//! NuttX runner for gallery's widgets showcase. The widget tree, theme
//! cycle, and slider→progress wiring come from `mirui::gallery::demos::widgets`;
//! this file adds the NuttX-only driver — syslog FPS sink, offscreen budget,
//! input feedback, and a scripted SimTimeline tour.

use mirui::anim::ease;
use mirui::app::{App, RendererFactory};
use mirui::ui::widgets::{Slider, Switch};
use mirui::ecs::Entity;
use mirui::input::event::sim::{SimAction, SimTimeline, sim_timeline_system};
use mirui::gallery::demos::widgets::{
    Cycle, ThemeCycleIndex, dark_with_accent, slider_to_progress_system,
};
use mirui::app::plugins::{FpsSummary, FpsSummaryPlugin, InputFeedbackPlugin, StdInstantClockPlugin};
use mirui::prelude::*;
use mirui::surface::Surface;
use mirui::types::DimPoint;
use mirui::ui::Children;

const DEFAULT_SCALE: i32 = 4;

pub const SIZE: (u16, u16) = ((128 * DEFAULT_SCALE) as u16, (128 * DEFAULT_SCALE) as u16);

pub fn build<S, F>(app: &mut App<S, F>) -> Entity
where
    S: Surface,
    F: RendererFactory<S>,
{
    let info = app.backend.display_info();
    let (view_w, view_h) = (info.width, info.height);

    app.add_plugin(InputFeedbackPlugin::default());
    app.add_plugin(StdInstantClockPlugin);
    app.add_plugin(FpsSummaryPlugin::default().with_sink(syslog_fps_sink));
    app.with_offscreen_pool_budget(512 * 1024);
    app.add_system(sim_timeline_system::system());
    app.add_system(slider_to_progress_system::system());

    app.world.insert_resource(dark_with_accent());
    let cycle_e = Cycle::install(&mut app.world);
    app.world.insert(cycle_e, ThemeCycleIndex(0));

    let root = WidgetBuilder::new(&mut app.world)
        .bg_color(ColorToken::Surface)
        .layout(LayoutStyle {
            direction: FlexDirection::Column,
            width: Dimension::px(view_w as i32),
            height: Dimension::px(view_h as i32),
            ..Default::default()
        })
        .id();

    mirui::gallery::demos::widgets::build_widgets(&mut app.world, root, view_w, view_h);

    if std::env::var("MIRUI_SIM_OFF").ok().as_deref() == Some("1") {
        return root;
    }

    let tab_kids = {
        let q: Vec<Entity> = app.world.query::<mirui::ui::widgets::TabBar>().collect();
        let tab_bar_e = *q.first().expect("TabBar must be installed");
        app.world
            .get::<Children>(tab_bar_e)
            .map(|c| c.0.clone())
            .unwrap_or_default()
    };
    let (tab_list, tab_form, tab_theme) = (tab_kids[0], tab_kids[1], tab_kids[2]);
    let switch_e = *app
        .world
        .query::<Switch>()
        .collect()
        .first()
        .expect("form Switch must be installed");
    let slider_e = *app
        .world
        .query::<Slider>()
        .collect()
        .first()
        .expect("form Slider must be installed");
    let list_drag_anchor = app
        .world
        .query::<mirui::ui::widgets::LazyList>()
        .collect()
        .first()
        .copied()
        .expect("list must be installed");

    app.world.insert_resource(
        SimTimeline::new(vec![
            SimAction::wait(800),
            SimAction::tap(DimPoint::CENTER).on(tab_form),
            SimAction::wait(800),
            SimAction::tap(DimPoint::CENTER).on(switch_e),
            SimAction::wait(800),
            SimAction::drag(
                DimPoint::percent(10, 50),
                DimPoint::percent(90, 50),
                600,
                ease::ease_in_out_cubic,
            )
            .on(slider_e),
            SimAction::wait(800),
            SimAction::tap(DimPoint::CENTER).on(switch_e),
            SimAction::wait(1500),
            SimAction::tap(DimPoint::CENTER).on(tab_theme),
            SimAction::wait(6500),
            SimAction::tap(DimPoint::CENTER).on(tab_list),
            SimAction::wait(800),
            SimAction::drag(
                DimPoint::percent(50, 80),
                DimPoint::percent(50, 20),
                100,
                ease::linear,
            )
            .on(list_drag_anchor),
            SimAction::wait(800),
            SimAction::drag(
                DimPoint::percent(50, 20),
                DimPoint::percent(50, 80),
                100,
                ease::linear,
            )
            .on(list_drag_anchor),
            SimAction::wait(800),
        ])
        .looping(true),
    );

    root
}

fn syslog_fps_sink(r: FpsSummary<'_>) {
    let work_fps = if r.avg_frame_ns == 0 {
        0.0
    } else {
        1_000_000_000.0 / r.avg_frame_ns as f64
    };
    let wall_fps = match r.wall_ns {
        Some(ns) if ns > 0 => f64::from(r.frames) * 1_000_000_000.0 / ns as f64,
        _ => 0.0,
    };
    mirui::__mirui_nuttx_info!(
        "[fps] {} frames | wall {:.1} fps | work {}us ({:.1} fps) = input {} + systems {} + layout {} + render {} + flush {} + seed {}",
        r.frames,
        wall_fps,
        r.avg_frame_ns / 1000,
        work_fps,
        r.avg_input_ns / 1000,
        r.avg_systems_ns / 1000,
        r.avg_layout_ns / 1000,
        r.avg_render_ns / 1000,
        r.avg_flush_ns / 1000,
        r.avg_seed_prev_ns / 1000,
    );
    if let Some(s) = r.stats {
        mirui::__mirui_nuttx_info!(
            "[fps] window={} min {}us max {}us p99 {}us jitter {}us",
            s.len(),
            s.min() / 1000,
            s.max() / 1000,
            s.p99() / 1000,
            s.jitter() / 1000,
        );
    }
}
