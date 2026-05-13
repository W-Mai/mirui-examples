//! Free-flying butterfly with asymmetric flutter. Every frame:
//!   - position follows a Lissajous figure (x and y use different periods so
//!     the trajectory is non-periodic-looking)
//!   - wing flap uses |sin| with a plateau so the motion is fast-fast-pause
//!     instead of smooth, matching the "fluttering flight" description
//!   - body leans slightly based on trajectory tangent so the butterfly
//!     "banks" into turns
//!   - each side of the butterfly is a single fill_path (not two), so there
//!     are no seams between forewing and hindwing; body and antennae are
//!     strokes on top
//!
//! Reference: Wikipedia "Butterfly" § Behaviour — "bumpy and unpredictable
//! flight … caused by the turbulence created by the small whirlpools formed
//! by the wings during flight."

use mirui::surface::framebuf::FramebufSurface;
use mirui::surface::{Surface, FramebufferAccess};
use mirui::draw::canvas::Canvas;
use mirui::draw::path::Path;
use mirui::draw::sw_backend::SwRenderer;
use mirui::types::{Color, Fixed, Point, Rect};

use crate::board::{H, W, systimer_now};

pub struct ButterflyDemo {
    start: u32,
}

impl ButterflyDemo {
    pub fn new() -> Self {
        Self {
            start: systimer_now(),
        }
    }

    pub fn step<F: FnMut(&[u8], &Rect)>(&mut self, fb: &mut FramebufSurface<F>) {
        {
            let tex = fb.framebuffer();
            let mut backend = SwRenderer::new(tex);

            let full = Rect::new(0, 0, W, H);
            backend.clear(&full, &Color::rgb(18, 22, 34));

            let elapsed_ms = (systimer_now().wrapping_sub(self.start) / 160_000) as i32;

            // --- Trajectory (Lissajous, coprime-ish periods so it doesn't
            // visibly repeat at real-world viewing durations) ---
            // x: 3.1 s period, y: 1.9 s period. Using integer ms arithmetic.
            let tx_deg = Fixed::from_int((elapsed_ms * 360 / 3100) % 360);
            let ty_deg = Fixed::from_int((elapsed_ms * 360 / 1900) % 360);
            let amp_x = Fixed::from_int(32);
            let amp_y = Fixed::from_int(28);
            let cx = Fixed::from_int(W as i32 / 2) + Fixed::sin_deg(tx_deg) * amp_x;
            let cy = Fixed::from_int(H as i32 / 2) + Fixed::sin_deg(ty_deg) * amp_y;

            // --- Body lean follows trajectory tangent (dx/dt scale) ---
            let tilt = Fixed::cos_deg(tx_deg) * Fixed::from_f32(0.35);

            // --- Yaw: slow rotation of the butterfly's facing direction.
            // Produces a perspective-like effect where one wing foreshortens
            // while the other opens up, simulating the body turning in air.
            // Period ~ 2.4 s, so one full "turn" every ~2.4 s. ---
            let yaw_deg = Fixed::from_int((elapsed_ms * 360 / 2400) % 360);
            let yaw = Fixed::sin_deg(yaw_deg) * Fixed::from_f32(0.55);

            // --- Flap with plateau: fast-fast-pause ---
            let flap_deg = Fixed::from_int((elapsed_ms * 360 / 280) % 360);
            let raw = Fixed::sin_deg(flap_deg).abs();
            let span_base = Fixed::from_f32(0.25) + raw * Fixed::from_f32(0.75);

            // Per-side span: yaw widens one side, narrows the other, giving
            // the foreshortening illusion. Clamp so neither side collapses
            // completely — the far wing stays visible as a thin silhouette.
            let min_span = Fixed::from_f32(0.15);
            let span_left = (span_base * (Fixed::ONE + yaw)).max(min_span);
            let span_right = (span_base * (Fixed::ONE - yaw)).max(min_span);

            // --- Draw wings: outer first (both sides), then inner pattern ---
            draw_wing(&mut backend, &full, cx, cy, span_left, tilt, -1, false);
            draw_wing(&mut backend, &full, cx, cy, span_right, tilt, 1, false);
            draw_wing(&mut backend, &full, cx, cy, span_left, tilt, -1, true);
            draw_wing(&mut backend, &full, cx, cy, span_right, tilt, 1, true);

            // --- Body (stroked line, leans with tilt) ---
            let body_head = Point {
                x: cx + tilt * Fixed::from_int(6),
                y: cy - Fixed::from_int(14),
            };
            let body_tail = Point {
                x: cx - tilt * Fixed::from_int(6),
                y: cy + Fixed::from_int(16),
            };
            backend.draw_line(
                body_head,
                body_tail,
                &full,
                Fixed::from_int(2),
                &Color::rgb(30, 20, 40),
                255,
            );

            // --- Antennae ---
            backend.draw_line(
                body_head,
                Point {
                    x: body_head.x - Fixed::from_int(5),
                    y: body_head.y - Fixed::from_int(10),
                },
                &full,
                Fixed::ONE,
                &Color::rgb(50, 40, 60),
                220,
            );
            backend.draw_line(
                body_head,
                Point {
                    x: body_head.x + Fixed::from_int(5),
                    y: body_head.y - Fixed::from_int(10),
                },
                &full,
                Fixed::ONE,
                &Color::rgb(50, 40, 60),
                220,
            );
        }
        fb.flush(&Rect::new(0, 0, W, H));
    }
}

/// Build and fill one side of the butterfly. The outline covers both forewing
/// (upper lobe) and hindwing (lower lobe) as a single closed curve so there
/// is no visible seam where they meet. When `inner=true` a smaller copy in a
/// brighter colour is drawn on top to suggest wing venation/pattern.
fn draw_wing(
    backend: &mut SwRenderer,
    clip: &Rect,
    cx: Fixed,
    cy: Fixed,
    span: Fixed,
    tilt: Fixed,
    side: i32,
    inner: bool,
) {
    let s = Fixed::from_int(side);
    let shrink = if inner {
        Fixed::from_f32(0.6)
    } else {
        Fixed::ONE
    };

    // Body attachment run — two anchors top and bottom along the body axis.
    // Tilt shifts both anchors horizontally by the same small amount to match
    // the leaning body.
    let anchor_top = Point {
        x: cx + tilt * Fixed::from_int(4),
        y: cy - Fixed::from_int(8) * shrink,
    };
    let anchor_bot = Point {
        x: cx - tilt * Fixed::from_int(4),
        y: cy + Fixed::from_int(10) * shrink,
    };

    // Forewing tip: extends upward-outward. span compresses x only (like a
    // wing folding side-on).
    let forewing_tip = Point {
        x: cx + Fixed::from_int(44) * s * span * shrink,
        y: cy - Fixed::from_int(24) * shrink,
    };
    // Hindwing tip: lower and slightly less extended.
    let hindwing_tip = Point {
        x: cx + Fixed::from_int(34) * s * span * shrink,
        y: cy + Fixed::from_int(22) * shrink,
    };

    // Control points for the outline. Roughly:
    //   anchor_top → (lift up and out) → forewing_tip → (sweep back in) →
    //   (trailing-edge notch near body) → hindwing_tip → (inner curve) →
    //   anchor_bot → close up through body edge.
    let fw_out_c1 = Point {
        x: cx + Fixed::from_int(18) * s * span * shrink,
        y: anchor_top.y - Fixed::from_int(22) * shrink,
    };
    let fw_out_c2 = Point {
        x: cx + Fixed::from_int(52) * s * span * shrink,
        y: forewing_tip.y - Fixed::from_int(6) * shrink,
    };
    let fw_in_c1 = Point {
        x: cx + Fixed::from_int(46) * s * span * shrink,
        y: cy - Fixed::from_int(6) * shrink,
    };
    let fw_in_c2 = Point {
        x: cx + Fixed::from_int(14) * s * span * shrink,
        y: cy - Fixed::from_int(1) * shrink,
    };
    // Notch where forewing and hindwing meet — pulled towards the body to
    // create the classic butterfly silhouette indent.
    let notch = Point {
        x: cx + Fixed::from_int(10) * s * span * shrink,
        y: cy + Fixed::from_int(4) * shrink,
    };
    let hw_out_c1 = Point {
        x: cx + Fixed::from_int(36) * s * span * shrink,
        y: cy + Fixed::from_int(6) * shrink,
    };
    let hw_out_c2 = Point {
        x: cx + Fixed::from_int(40) * s * span * shrink,
        y: hindwing_tip.y - Fixed::from_int(2) * shrink,
    };
    let hw_in_c1 = Point {
        x: cx + Fixed::from_int(26) * s * span * shrink,
        y: cy + Fixed::from_int(18) * shrink,
    };
    let hw_in_c2 = Point {
        x: cx + Fixed::from_int(8) * s * span * shrink,
        y: cy + Fixed::from_int(14) * shrink,
    };

    let mut path = Path::new();
    path.move_to(anchor_top);
    path.cubic_to(fw_out_c1, fw_out_c2, forewing_tip);
    path.cubic_to(fw_in_c1, fw_in_c2, notch);
    path.cubic_to(hw_out_c1, hw_out_c2, hindwing_tip);
    path.cubic_to(hw_in_c1, hw_in_c2, anchor_bot);
    path.close();

    // Blue morpho-inspired palette: deep indigo outer wing with bright
    // cyan-teal inner pattern. Sides are very slightly different to hint
    // at iridescent colour shift.
    let color = if inner {
        if side < 0 {
            Color::rgb(130, 210, 240)
        } else {
            Color::rgb(150, 220, 245)
        }
    } else if side < 0 {
        Color::rgb(40, 70, 160)
    } else {
        Color::rgb(50, 80, 170)
    };
    let opa = if inner { 210 } else { 240 };
    backend.fill_path(&path, clip, &color, opa);
}
