use esp_hal::{
    dma::{DmaRxBuf, DmaTxBuf},
    dma_buffers,
    gpio::{Level, Output, OutputConfig},
    spi::{
        Mode,
        master::{Config, Spi},
    },
    time::Rate,
};
use mirui::{prelude::*, render::texture::ColorFormat, surface::framebuf::FramebufSurface};

#[path = "../../esp32c3-animation/src/board.rs"]
mod board;

pub async fn run(p: esp_hal::peripherals::Peripherals) {
    let (rx, rx_desc, tx, tx_desc) = dma_buffers!(64, 32000);
    let spi = Spi::new(
        p.SPI2,
        Config::default()
            .with_frequency(Rate::from_mhz(32))
            .with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(p.GPIO5)
    .with_mosi(p.GPIO4)
    .with_dma(p.DMA_CH0)
    .with_buffers(
        DmaRxBuf::new(rx_desc, rx).unwrap(),
        DmaTxBuf::new(tx_desc, tx).unwrap(),
    );
    let mut lcd = board::St7735 {
        spi,
        dc: Output::new(p.GPIO7, Level::Low, OutputConfig::default()),
        cs: Output::new(p.GPIO6, Level::High, OutputConfig::default()),
    };
    let mut reset = Output::new(p.GPIO3, Level::High, OutputConfig::default());
    let mut backlight = Output::new(p.GPIO2, Level::Low, OutputConfig::default());
    lcd.init(&mut reset);
    backlight.set_high();
    let surface = FramebufSurface::with_format(
        board::W,
        board::H,
        ColorFormat::RGB565Swapped,
        move |bytes: &[u8], rect: PhysicalRect| {
            if rect.width() != 0 && rect.height() != 0 {
                lcd.push_region_raw(
                    bytes,
                    board::W,
                    rect.x(),
                    rect.y(),
                    rect.width(),
                    rect.height(),
                );
            }
        },
    );
    let mut app = App::new(surface);
    app.with_default_widgets().with_default_systems();
    mirui::core::time::set_clock(clock);
    app.add_resource(MonoClock::from_time_source());
    let root = app.spawn_root().id();
    let session = mirui::gallery::demos::chord_prompter::install(&mut app, root);
    app.prepare_text_layout().expect("initial text capacity");
    app.render().expect("initial display");
    esp_println::println!("[DISPLAY] ready free={}", esp_alloc::HEAP.free());
    let prepare = core::cell::Cell::new(false);
    super::run_ble(p.BT, p.TIMG0, p.SW_INTERRUPT, session, &prepare, async {
        loop {
            if prepare.replace(false)
                && let Err(error) = app.prepare_text_layout()
            {
                esp_println::println!("[LAYOUT] {error:?}");
            }
            app.tick();
            embassy_time::Timer::after_millis(16).await;
        }
    })
    .await;
}

pub fn clock() -> u64 {
    esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_micros()
        .saturating_mul(1000)
}
