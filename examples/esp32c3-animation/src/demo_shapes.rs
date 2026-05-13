//! Direct Canvas-API demo — exercises draw_line and draw_arc against
//! the framebuffer every frame without going through App/ECS.
//!
//! Kept deliberately small: the per-pixel sqrt in fill_path's AA makes large
//! filled regions extremely slow on ESP32-C3 (tracked as a Quality TODO).

use mirui::surface::framebuf::FramebufSurface;
use mirui::surface::{Surface, FramebufferAccess};
use mirui::draw::canvas::Canvas;
use mirui::draw::sw_backend::SwRenderer;
use mirui::types::{Color, Fixed, Point, Rect};

use crate::board::{H, W, systimer_now};

pub struct ShapesDemo {
    start: u32,
}

impl ShapesDemo {
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
            backend.clear(&full, &Color::rgb(20, 20, 30));

            let center = Point {
                x: Fixed::from_int(W as i32 / 2),
                y: Fixed::from_int(H as i32 / 2),
            };
            let r = Fixed::from_int(40);
            let clip = full;

            backend.draw_arc(
                center,
                r,
                Fixed::from_int(0),
                Fixed::from_int(360),
                &clip,
                Fixed::from_int(2),
                &Color::rgb(80, 180, 220),
                255,
            );

            // Angle is wall-clock driven so the hand moves consistently
            // regardless of fps. 160_000_000 systimer ticks per second on
            // ESP32-C3; full revolution every 30 s = 12°/s.
            let now = systimer_now().wrapping_sub(self.start);
            let elapsed_ms = (now / 160_000) as i32;
            let angle_deg_raw = ((elapsed_ms * 360) / 30_000) % 360;
            let angle_deg = Fixed::from_int(angle_deg_raw);
            let end = Point {
                x: center.x + Fixed::cos_deg(angle_deg) * r,
                y: center.y + Fixed::sin_deg(angle_deg) * r,
            };
            backend.draw_line(
                center,
                end,
                &clip,
                Fixed::from_int(2),
                &Color::rgb(255, 180, 80),
                255,
            );

            // Tick marks around the circle.
            for i in 0..12 {
                let a = Fixed::from_int(i * 30);
                let inner = r - Fixed::from_int(5);
                let outer = r - Fixed::from_int(1);
                let p1 = Point {
                    x: center.x + Fixed::cos_deg(a) * inner,
                    y: center.y + Fixed::sin_deg(a) * inner,
                };
                let p2 = Point {
                    x: center.x + Fixed::cos_deg(a) * outer,
                    y: center.y + Fixed::sin_deg(a) * outer,
                };
                backend.draw_line(p1, p2, &clip, Fixed::ONE, &Color::rgb(180, 180, 200), 255);
            }
        }
        fb.flush(&Rect::new(0, 0, W, H));
    }
}
