use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::serial::{NativeSensorSource, SensorEvent, SerialEsp32Source};

pub trait DataSource: Send {
    fn name(&self) -> String;
    fn next_event(&mut self) -> Result<SensorEvent, String>;
}

pub struct SerialEsp32DataSource {
    source: SerialEsp32Source,
    feature_mapping: Arc<Mutex<BTreeMap<String, String>>>,
}

impl SerialEsp32DataSource {
    pub fn open(
        port: &str,
        baud: u32,
        feature_mapping: Arc<Mutex<BTreeMap<String, String>>>,
    ) -> Result<Self, String> {
        Ok(Self {
            source: SerialEsp32Source::open(port, baud)?,
            feature_mapping,
        })
    }
}

impl DataSource for SerialEsp32DataSource {
    fn name(&self) -> String {
        self.source.describe()
    }

    fn next_event(&mut self) -> Result<SensorEvent, String> {
        loop {
            let mapping = self
                .feature_mapping
                .lock()
                .map_err(|_| "Feature mapping lock poisoned".to_string())?
                .clone();
            match self.source.next_event(&mapping)? {
                Some(event) => return Ok(event),
                None => continue,
            }
        }
    }
}

pub struct NativeSensorDataSource {
    source: NativeSensorSource,
}

impl NativeSensorDataSource {
    pub fn new() -> Self {
        Self {
            source: NativeSensorSource::new(),
        }
    }
}

impl DataSource for NativeSensorDataSource {
    fn name(&self) -> String {
        "native-sensor-placeholder".to_string()
    }

    fn next_event(&mut self) -> Result<SensorEvent, String> {
        self.source.next_event()
    }
}

