use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
enum SensorKind {
    Mq7,
    Dht22,
    Adc,
    Pcf8591,
}

#[derive(Clone, Debug)]
struct SensorDriver {
    kind: SensorKind,
    name: &'static str,
}

fn detect_all_sensors() -> Vec<SensorDriver> {
    let mut sensors = Vec::new();

    // Replace these stubs with real hardware probing on ESP32.
    // Keep names aligned with gateway parser: mq7, dht22, adc, pcf8591.
    sensors.push(SensorDriver {
        kind: SensorKind::Mq7,
        name: "mq7",
    });
    sensors.push(SensorDriver {
        kind: SensorKind::Dht22,
        name: "dht22",
    });
    sensors.push(SensorDriver {
        kind: SensorKind::Adc,
        name: "adc",
    });
    sensors.push(SensorDriver {
        kind: SensorKind::Pcf8591,
        name: "pcf8591",
    });

    sensors
}

fn emit_aiag_hello(sensors: &[SensorDriver]) {
    println!("AIAG HELLO fw=rust-sensor-node ver=1");
    let mut names: Vec<&str> = sensors.iter().map(|s| s.name).collect();
    names.sort_unstable();
    println!("AIAG CAPS sensors={}", names.join(","));
    println!("AIAG RUN state=streaming");
}

fn sample_line(sensor: &SensorDriver, t: f32) -> String {
    match sensor.kind {
        SensorKind::Mq7 => {
            let raw = 200 + ((t.sin() + 1.0) * 20.0) as u16;
            let voltage = raw as f32 * 3.3 / 4095.0;
            format!("MQ7 raw={} voltage={:.3}V", raw, voltage)
        }
        SensorKind::Dht22 => {
            let temp = 25.0 + t.sin() * 3.0;
            let hum = 60.0 + t.cos() * 8.0;
            format!("DHT22 temp_c={:.1} hum={:.1}", temp, hum)
        }
        SensorKind::Adc => {
            let raw = 500 + ((t.cos() + 1.0) * 50.0) as u16;
            let voltage = raw as f32 * 3.3 / 4095.0;
            format!("ADC pin=34 raw={} voltage={:.3}V", raw, voltage)
        }
        SensorKind::Pcf8591 => {
            let base = ((t.sin() + 1.0) * 80.0) as u8;
            format!(
                "PCF8591 addr=0x48 AIN0={} AIN1={} AIN2={} AIN3={}",
                base,
                base.saturating_add(10),
                base.saturating_add(20),
                base.saturating_add(30)
            )
        }
    }
}

fn main() {
    let sensors = detect_all_sensors();

    // The gateway marks managed devices by seeing AIAG* lines.
    emit_aiag_hello(&sensors);

    // Emit initial lines within discovery window so gateway can capture capabilities.
    let warmup = Instant::now();
    while warmup.elapsed() < Duration::from_millis(1400) {
        let t = warmup.elapsed().as_secs_f32();
        for sensor in &sensors {
            println!("{}", sample_line(sensor, t));
        }
        thread::sleep(Duration::from_millis(250));
    }

    loop {
        let t = warmup.elapsed().as_secs_f32();
        for sensor in &sensors {
            println!("{}", sample_line(sensor, t));
        }
        thread::sleep(Duration::from_millis(1000));
    }
}
