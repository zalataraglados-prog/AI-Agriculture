use chrono::{Local, SecondsFormat};

pub(crate) fn now_rfc3339() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Millis, false)
}
