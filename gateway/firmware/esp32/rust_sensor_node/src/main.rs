#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::Flex;
use esp_hal::main;
use esp_println::println;

const DHT22_DEBUG_TIMEOUT: bool = true;

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let delay = Delay::new();

    // Fixed single-source read: DHT22 DATA on GPIO4.
    let mut dht22_pin = Flex::new(peripherals.GPIO4);

    println!("AIAG HELLO fw=rust-sensor-node ver=5");
    println!("AIAG CAPS mode=single");
    println!("AIAG CAPS sensor=dht22");

    // DHT22 needs a short warm-up window after boot.
    delay.delay_millis(2000);

    loop {
        if let Some((temp_c, hum)) = read_dht22(&mut dht22_pin, &delay) {
            let temp_int = temp_c / 10;
            let temp_dec = (temp_c % 10).abs();
            let hum_int = hum / 10;
            let hum_dec = (hum % 10).abs();
            println!(
                "DHT22 temp_c={}.{} hum={}.{}",
                temp_int,
                temp_dec,
                hum_int,
                hum_dec
            );
        } else {
            println!("AIAG WARN sensor=dht22 status=read_failed");
        }

        delay.delay_millis(5000);
    }
}

fn read_dht22(pin: &mut Flex<'_>, delay: &Delay) -> Option<(i16, i16)> {
    pin.set_input_enable(false);
    pin.set_output_enable(true);
    pin.set_low();
    delay.delay_millis(20);
    pin.set_high();
    delay.delay_micros(30);

    pin.set_output_enable(false);
    pin.set_input_enable(true);

    if DHT22_DEBUG_TIMEOUT {
        println!(
            "AIAG SCOPE idle={} pin=4",
            if pin.is_low() { "low" } else { "high" }
        );
    }

    let resp_low_wait = match wait_for_level_timed(pin, false, delay, 120) {
        Some(v) => v,
        None => {
            log_wait_timeout("resp_low", false, pin, 120);
            return None;
        }
    };
    let resp_low_width = match wait_for_level_timed(pin, true, delay, 120) {
        Some(v) => v,
        None => {
            log_wait_timeout("resp_high", true, pin, 120);
            return None;
        }
    };
    let resp_high_width = match wait_for_level_timed(pin, false, delay, 120) {
        Some(v) => v,
        None => {
            log_wait_timeout("resp_low_2", false, pin, 120);
            return None;
        }
    };

    if DHT22_DEBUG_TIMEOUT {
        println!(
            "AIAG SCOPE resp wait_low_us={} low_us={} high_us={}",
            resp_low_wait,
            resp_low_width,
            resp_high_width
        );
    }

    let mut bit_high_us = [0u16; 8];
    let mut bit_sample = [0u8; 8];
    let mut data = [0u8; 5];
    for idx in 0..40 {
        if !wait_for_level(pin, true, delay, 80) {
            log_wait_timeout("bit_high", true, pin, 80);
            return None;
        }

        let mut high_elapsed = 0u16;
        let mut sampled = false;
        let mut bit = 0u8;
        while high_elapsed < 100 {
            if !sampled && high_elapsed >= 30 {
                bit = if pin.is_high() { 1 } else { 0 };
                sampled = true;
            }
            if pin.is_low() {
                break;
            }
            delay.delay_micros(2);
            high_elapsed += 2;
        }
        if !sampled {
            bit = if pin.is_high() { 1 } else { 0 };
        }
        if high_elapsed >= 100 && pin.is_high() {
            log_wait_timeout("bit_low", false, pin, 100);
            return None;
        }

        data[idx / 8] = (data[idx / 8] << 1) | bit;
        if idx < 8 {
            bit_high_us[idx] = high_elapsed;
            bit_sample[idx] = bit;
        }
    }

    if DHT22_DEBUG_TIMEOUT {
        println!(
            "AIAG SCOPE b0_us={} b1_us={} b2_us={} b3_us={} b4_us={} b5_us={} b6_us={} b7_us={} bits={}{}{}{}{}{}{}{}",
            bit_high_us[0],
            bit_high_us[1],
            bit_high_us[2],
            bit_high_us[3],
            bit_high_us[4],
            bit_high_us[5],
            bit_high_us[6],
            bit_high_us[7],
            bit_sample[0],
            bit_sample[1],
            bit_sample[2],
            bit_sample[3],
            bit_sample[4],
            bit_sample[5],
            bit_sample[6],
            bit_sample[7]
        );
    }

    // Prevent false-positive frames where all payload bits are zero.
    if data[0] == 0 && data[1] == 0 && data[2] == 0 && data[3] == 0 {
        if DHT22_DEBUG_TIMEOUT {
            println!("AIAG DBG dht22_frame all_zero");
        }
        return None;
    }

    let checksum = data[0]
        .wrapping_add(data[1])
        .wrapping_add(data[2])
        .wrapping_add(data[3]);
    if checksum != data[4] {
        if DHT22_DEBUG_TIMEOUT {
            println!(
                "AIAG DBG dht22_checksum fail sum={} recv={} b0={} b1={} b2={} b3={}",
                checksum,
                data[4],
                data[0],
                data[1],
                data[2],
                data[3]
            );
        }
        return None;
    }

    let hum_raw = u16::from(data[0]) << 8 | u16::from(data[1]);
    let temp_raw = u16::from(data[2]) << 8 | u16::from(data[3]);

    let hum = hum_raw as i16;
    let temp = if (temp_raw & 0x8000) != 0 {
        -((temp_raw & 0x7FFF) as i16)
    } else {
        temp_raw as i16
    };

    Some((temp, hum))
}

fn wait_for_level_timed(
    pin: &mut Flex<'_>,
    target_low: bool,
    delay: &Delay,
    timeout_us: u32,
) -> Option<u32> {
    let mut elapsed = 0u32;
    while elapsed < timeout_us {
        if pin.is_low() == target_low {
            return Some(elapsed);
        }
        delay.delay_micros(2);
        elapsed += 2;
    }
    None
}

fn wait_for_level(pin: &mut Flex<'_>, target_low: bool, delay: &Delay, timeout_us: u32) -> bool {
    let mut elapsed = 0u32;
    while elapsed < timeout_us {
        let is_low = pin.is_low();
        if is_low == target_low {
            return true;
        }
        delay.delay_micros(2);
        elapsed += 2;
    }
    false
}

fn log_wait_timeout(stage: &str, target_low: bool, pin: &mut Flex<'_>, timeout_us: u32) {
    if DHT22_DEBUG_TIMEOUT {
        let last = if pin.is_low() { "low" } else { "high" };
        println!(
            "AIAG DBG dht22_timeout stage={} expect={} last={} timeout_us={}",
            stage,
            if target_low { "low" } else { "high" },
            last,
            timeout_us
        );
    }
}
