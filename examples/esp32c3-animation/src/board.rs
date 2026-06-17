#[cfg(feature = "fps-overlay")]
use alloc::vec;
use embedded_hal::spi::SpiBus;
use esp_hal::gpio::Output;

pub const W: u16 = 128;
pub const H: u16 = 128;

pub struct St7735<'a, S: SpiBus<u8>> {
    pub spi: S,
    pub dc: Output<'a>,
    pub cs: Output<'a>,
}

impl<'a, S: SpiBus<u8>> St7735<'a, S> {
    pub fn cmd(&mut self, c: u8) {
        self.cs.set_low();
        self.dc.set_low();
        self.spi.write(&[c]).ok();
        self.cs.set_high();
    }

    pub fn data(&mut self, d: &[u8]) {
        self.cs.set_low();
        self.dc.set_high();
        self.spi.write(d).ok();
        self.cs.set_high();
    }

    pub fn init(&mut self, rst: &mut Output) {
        rst.set_low();
        delay_ms(10);
        rst.set_high();
        delay_ms(120);
        self.cmd(0x01);
        delay_ms(150);
        self.cmd(0x11);
        delay_ms(500);
        self.cmd(0xB1);
        self.data(&[0x01, 0x01, 0x01]);
        self.cmd(0xB2);
        self.data(&[0x01, 0x01, 0x01]);
        self.cmd(0xB3);
        self.data(&[0x01, 0x01, 0x01, 0x01, 0x01, 0x01]);
        self.cmd(0xB4);
        self.data(&[0x07]);
        self.cmd(0xC0);
        self.data(&[0xA2, 0x02, 0x84]);
        self.cmd(0xC1);
        self.data(&[0xC5]);
        self.cmd(0xC2);
        self.data(&[0x0A, 0x00]);
        self.cmd(0xC3);
        self.data(&[0x8A, 0x2A]);
        self.cmd(0xC4);
        self.data(&[0x8A, 0xEE]);
        self.cmd(0xC5);
        self.data(&[0x0E]);
        self.cmd(0x20);
        self.cmd(0x36);
        self.data(&[0xC8]);
        self.cmd(0x3A);
        self.data(&[0x05]);
        self.cmd(0x13);
        delay_ms(10);
        self.cmd(0x29);
        delay_ms(100);
    }

    pub fn set_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) {
        let (xo, yo) = (2u16, 3u16);
        self.cmd(0x2A);
        self.data(&[
            ((x0 + xo) >> 8) as u8,
            (x0 + xo) as u8,
            ((x1 + xo) >> 8) as u8,
            (x1 + xo) as u8,
        ]);
        self.cmd(0x2B);
        self.data(&[
            ((y0 + yo) >> 8) as u8,
            (y0 + yo) as u8,
            ((y1 + yo) >> 8) as u8,
            (y1 + yo) as u8,
        ]);
        self.cmd(0x2C);
    }

    pub fn push_region_raw(&mut self, rgb565: &[u8], fb_w: u16, x: u16, y: u16, w: u16, h: u16) {
        const BATCH_ROWS: usize = 16;
        self.set_window(x, y, x + w - 1, y + h - 1);
        self.cs.set_low();
        self.dc.set_high();
        let stride = fb_w as usize * 2;
        let mut row = 0usize;
        while row < h as usize {
            let rows_this_batch = BATCH_ROWS.min(h as usize - row);
            for r in 0..rows_this_batch {
                let src_off = (y as usize + row + r) * stride + x as usize * 2;
                self.spi.write(&rgb565[src_off..src_off + w as usize * 2]).ok();
            }
            row += rows_this_batch;
        }
        self.cs.set_high();
    }
}

pub fn delay_ms(ms: u32) {
    for _ in 0..ms {
        for _ in 0..16_000u32 {
            core::hint::spin_loop();
        }
    }
}

/// Vendor-specific CSR 0x7E2 — clock frequency assumption was wrong
/// historically (assumed 160 MHz). Use `esp_hal::time::Instant::now()`
/// for any wall-clock measurement that must agree with `MonoClock`.
/// Kept for legacy demos that compare deltas (relative monotonic
/// counter, not absolute time).
#[allow(dead_code)]
pub fn systimer_now() -> u32 {
    let val: u32;
    unsafe { core::arch::asm!("csrr {}, 0x7E2", out(reg) val) };
    val
}

#[cfg(feature = "fps-overlay")]
pub fn draw_perf_overlay<S: SpiBus<u8>>(
    lcd: &mut St7735<S>,
    fps: u32,
    budget_violations: u32,
) {
    // RGB565 swapped (LSB-first byte order over SPI):
    //   yellow ≈ 0xFFE0 little-endian → bytes E0 FF
    //   red    ≈ 0xF800 little-endian → bytes 00 F8
    draw_overlay_line(lcd, 2, fps, b'f', [0xE0, 0xFF]);
    if budget_violations != 0 {
        draw_overlay_line(lcd, 12, budget_violations, b'!', [0x00, 0xF8]);
    }
}

#[cfg(feature = "fps-overlay")]
fn draw_overlay_line<S: SpiBus<u8>>(
    lcd: &mut St7735<S>,
    sy: u16,
    value: u32,
    suffix: u8,
    px: [u8; 2],
) {
    let mut num = [0u8; 8];
    let mut len = 0;
    let mut n = value;
    if n == 0 {
        num[0] = b'0';
        len = 1;
    } else {
        while n > 0 && len < 5 {
            num[len] = b'0' + (n % 10) as u8;
            n /= 10;
            len += 1;
        }
        num[..len].reverse();
    }
    num[len] = suffix;
    len += 1;

    let fw: u16 = (len as u16) * 8;
    let fh: u16 = 8;
    let sx = W - fw - 2;
    lcd.set_window(sx, sy, sx + fw - 1, sy + fh - 1);
    lcd.cs.set_low();
    lcd.dc.set_high();
    let mut row_buf = vec![0u8; fw as usize * 2];
    for row in 0..fh as usize {
        for col in 0..fw as usize {
            let ci = col / 8;
            let bit = col % 8;
            let glyph = mirui::render::font::glyph(num[ci]);
            let on = glyph[row] & (0x80 >> bit) != 0;
            row_buf[col * 2] = if on { px[0] } else { 0x00 };
            row_buf[col * 2 + 1] = if on { px[1] } else { 0x00 };
        }
        lcd.spi.write(&row_buf).ok();
    }
    lcd.cs.set_high();
}
