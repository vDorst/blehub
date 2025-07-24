use embassy_time::{Duration, Timer};
use embedded_graphics::{
    mono_font::{MonoTextStyle, MonoTextStyleBuilder, ascii},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};

use embedded_hal::digital::OutputPin;
use embedded_hal_async::spi::SpiDevice;
use embedded_hal_bus::spi::ExclusiveDevice;

use esp_hal::gpio::Output;
use ssd1306::{Ssd1306, Ssd1306Async, prelude::*, size::DisplaySize128x64};

pub type DspSpi<O> = ExclusiveDevice<
    esp_hal::spi::master::SpiDmaBus<'static, esp_hal::Async>,
    O,
    embassy_time::Delay,
>;

pub type Dsp<O> = Ssd1306Async<
    SPIInterface<&'static mut DspSpi<O>, O>,
    DisplaySize128x64,
    ssd1306::mode::BufferedGraphicsModeAsync<DisplaySize128x64>,
>;

pub async fn init_display<SPI: SpiDevice, O: OutputPin>(
    spi: &'static mut DspSpi<O>,
    lcd_spi_dcx: O,
    mut lcd_rst: O,
) -> Result<Dsp<O>, ()> {
    lcd_rst.set_low().unwrap();

    Timer::after_millis(10).await;

    lcd_rst.set_high().unwrap();

    let interface = SPIInterface::new(spi, lcd_spi_dcx);
    let mut display = Ssd1306Async::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    // display
    display.init().await.unwrap();

    let text_style = MonoTextStyleBuilder::new()
        .font(&ascii::FONT_5X8)
        .text_color(BinaryColor::On)
        .background_color(BinaryColor::Off)
        .build();

    Text::with_baseline("Hello Rust!", Point::new(0, 1), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();

    display.flush().await.unwrap();

    Ok(display)
}

#[embassy_executor::task]
pub async fn display_task(mut display: Dsp<Output<'static>>) {
    let style = MonoTextStyle::new(&ascii::FONT_8X13, BinaryColor::On);

    Text::new("Rust", Point::new(0, 0), style)
        .draw(&mut display)
        .unwrap();

    display.flush().await.unwrap();

    let mut step = 0;

    // loop
    loop {
        step += 1;
        Timer::after(Duration::from_millis(1_000)).await;

        let color = if step & 0x01 != 0x00 {
            BinaryColor::On
        } else {
            BinaryColor::Off
        };

        Pixel(Point::new(0, 0), color).draw(&mut display).unwrap();

        display.flush().await.unwrap();
    }
}
