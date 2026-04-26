use crate::serial::{ModbusConfig, SensorEvent, SerialEsp32Source};

pub trait DataSource: Send {
    fn name(&self) -> String;
    fn next_event(&mut self) -> Result<SensorEvent, String>;
}

pub struct SerialEsp32DataSource {
    source: SerialEsp32Source,
}

impl SerialEsp32DataSource {
    pub fn open(port: &str, baud: u32, modbus: &ModbusConfig) -> Result<Self, String> {
        Ok(Self {
            source: SerialEsp32Source::open(port, baud, modbus)?,
        })
    }
}

impl DataSource for SerialEsp32DataSource {
    fn name(&self) -> String {
        self.source.describe()
    }

    fn next_event(&mut self) -> Result<SensorEvent, String> {
        self.source.next_event()
    }
}
