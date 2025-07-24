#![no_std]
#![no_main]
#![allow(unused_imports)]
// #![feature(try_blocks)]
#![allow(dead_code)]
#![allow(unused_variables)]

#[cfg(feature = "defmt")]
use defmt::*;

use embassy_sync::signal::Signal;
use embassy_time::Timer;
use esp_backtrace as _;

mod fmt;

mod ble;
mod display;
mod esp32;
mod temperature;

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::mutex::Mutex;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, PrimitiveStyle};
use embedded_hal::spi::Mode;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::gpio::{Output, OutputConfig};
use esp_hal::i2c::master::{Config as I2C_Config, I2c};
use esp_hal::spi::master::{Config, Spi};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{Async, dma_buffers};
use esp_wifi::ble::controller::BleConnector;
use static_cell::StaticCell;
use trouble_host::prelude::ExternalController;

esp_bootloader_esp_idf::esp_app_desc!();

static DSP_SPI: StaticCell<display::DspSpi<Output<'static>>> = StaticCell::new();
static TEMP_I2C: StaticCell<I2c<'static, Async>> = StaticCell::new();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_println::println!("Init!");
    let peripherals = esp_hal::init(esp_hal::Config::default());

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let systimer = esp_hal::timer::systimer::SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(systimer.alarm0);

    let mut rng = esp_hal::rng::Trng::new(peripherals.RNG, peripherals.ADC1);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let init = esp_wifi::init(timg0.timer0, rng.rng.clone()).unwrap();
    let bluetooth = peripherals.BT;
    let connector = BleConnector::new(&init, bluetooth);
    let controller: ExternalController<_, 20> = ExternalController::new(connector);

    // Temp Sensor I2C
    let i2c = I2c::new(peripherals.I2C0, I2C_Config::default())
        .unwrap()
        .with_scl(peripherals.GPIO7)
        .with_sda(peripherals.GPIO6)
        .into_async();

    // Display GPIO
    let (dsp_spi, dsp_rst, dsp_dc) = {
        // DSP DO = SCLK
        let sclk = peripherals.GPIO23;
        // DSP D1 = SDO
        let mosi = peripherals.GPIO22;
        let res = peripherals.GPIO21;
        let dc = peripherals.GPIO20;
        let cs = peripherals.GPIO19;

        let dsp_reset = Output::new(res, esp_hal::gpio::Level::High, OutputConfig::default());
        let dsp_dc = Output::new(dc, esp_hal::gpio::Level::Low, OutputConfig::default());
        let dsp_cs = Output::new(cs, esp_hal::gpio::Level::High, OutputConfig::default());

        let dma_channel = peripherals.DMA_CH0;
        let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(2048);
        let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
        let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

        let spi_bus = Spi::new(
            peripherals.SPI2,
            Config::default()
                .with_frequency(Rate::from_khz(1000))
                .with_mode(esp_hal::spi::Mode::_0),
        )
        .unwrap()
        .with_sck(sclk)
        .with_mosi(mosi)
        .with_dma(dma_channel)
        .with_buffers(dma_rx_buf, dma_tx_buf)
        .into_async();

        let spi_dev: ExclusiveDevice<
            esp_hal::spi::master::SpiDmaBus<'_, esp_hal::Async>,
            Output<'_>,
            embassy_time::Delay,
        > = ExclusiveDevice::new(spi_bus, dsp_cs, embassy_time::Delay).unwrap();

        let spi_dev = DSP_SPI.init(spi_dev);

        (spi_dev, dsp_reset, dsp_dc)
    };

    let dsp = display::init_display::<display::DspSpi<Output<'static>>, Output<'_>>(
        dsp_spi, dsp_dc, dsp_rst,
    )
    .await
    .unwrap();

    esp_println::println!("Spawn: Temp");
    spawner.spawn(temperature::task_temperature(i2c)).unwrap();

    esp_println::println!("Spawn: Display");
    spawner.spawn(display::display_task(dsp)).unwrap();

    esp_println::println!("Spawn: Ble");

    ble::run(controller, &mut rng).await;
}
