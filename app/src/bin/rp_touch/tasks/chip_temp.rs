use embassy_rp::{
    adc::{Adc, Blocking, Channel, Config},
    peripherals,
    Peri,
};
use embassy_time::{Duration, Timer};

use crate::shared::{CHIP_TEMP_SAMPLE_PERIOD_MS, CHIP_TEMP_WATCH};

const ADC_TO_VOLTS: f32 = 3.3 / 4096.0;
const TS_AT_27C_VOLTS: f32 = 0.706;
const TS_SLOPE_VOLTS_PER_C: f32 = 0.001_721;

fn raw_to_celsius(raw: u16) -> i32 {
    let sensed_volts = raw as f32 * ADC_TO_VOLTS;
    let temp_c = 27.0 - ((sensed_volts - TS_AT_27C_VOLTS) / TS_SLOPE_VOLTS_PER_C);
    if temp_c >= 0.0 {
        (temp_c + 0.5) as i32
    } else {
        (temp_c - 0.5) as i32
    }
}

#[embassy_executor::task]
pub async fn chip_temp_task(
    adc_peripheral: Peri<'static, peripherals::ADC>,
    adc_temp_sensor: Peri<'static, peripherals::ADC_TEMP_SENSOR>,
) -> ! {
    let mut adc = Adc::<Blocking>::new_blocking(adc_peripheral, Config::default());
    let mut temp_sensor = Channel::new_temp_sensor(adc_temp_sensor);
    let sender = CHIP_TEMP_WATCH.sender();

    let mut filtered_temp_c = 0i32;
    let mut has_seed = false;

    loop {
        if let Ok(raw) = adc.blocking_read(&mut temp_sensor) {
            let sample_c = raw_to_celsius(raw);
            if has_seed {
                // Light smoothing to avoid 1-degree flicker on UI.
                filtered_temp_c = ((filtered_temp_c * 3) + sample_c) / 4;
            } else {
                filtered_temp_c = sample_c;
                has_seed = true;
            }
            sender.send(filtered_temp_c);
        }

        Timer::after(Duration::from_millis(CHIP_TEMP_SAMPLE_PERIOD_MS)).await;
    }
}
