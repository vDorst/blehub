use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Ticker, Timer};
use esp_hal::{Async, i2c::master::I2c};
use esp_println::println;
use sht3x::SHT3x;

#[derive(Debug)]
pub struct TempData {
    pub t: sht3x::Tmp,
    pub h: sht3x::Hum,
}

type TempSignal = Signal<CriticalSectionRawMutex, TempData>;
pub static TEMPDATA: TempSignal = TempSignal::new();

#[embassy_executor::task]
pub async fn task_temperature(mut i2c: I2c<'static, Async>) {
    let mut sht = SHT3x::new(&mut i2c);

    let _ = sht.reset().await;
    let mut tick = Ticker::every(Duration::from_secs(5));
    Timer::after(Duration::from_millis(10)).await;

    sht.write(sht3x::CMD::AUTO_1MPS_HIGH).await.unwrap();

    Timer::after(Duration::from_millis(100)).await;

    loop {
        match sht.get_measurement().await {
            Ok((t, h)) => {
                let hum: u8 = h.into();
                let tmp: i16 = t.into();
                println!("t: {}, h: {}%", tmp, hum);

                TEMPDATA.signal(TempData { t, h });
            }
            Err(e) => {
                println!("TEMP: I2C error: {:?}", e);
            }
        }

        tick.next().await;
    }
}
