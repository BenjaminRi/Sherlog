extern crate chrono;

use chrono::prelude::*;

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum LogLevel {
	Critical,
    Error,
    Warning,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
	pub fn to_string(&self) -> String {
		match self {
			LogLevel::Critical => "Critical".to_string(),
			LogLevel::Error => "Error".to_string(),
			LogLevel::Warning => "Warning".to_string(),
			LogLevel::Info => "Info".to_string(),
			LogLevel::Debug => "Debug".to_string(),
			LogLevel::Trace => "Trace".to_string(),
		}
	}
}

pub struct LogEntry {
	pub timestamp : chrono::DateTime<Utc>,
	pub severity : LogLevel,
	pub message : String,
}

pub enum LogSourceContents {
	Sources(Vec::<LogSource>),
	Entries(Vec::<LogEntry>),
}

pub struct LogSource {
	pub name : String,
	pub children : LogSourceContents,
}

impl Default for LogEntry {
    fn default() -> LogEntry {
        LogEntry {
            timestamp: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
            severity : LogLevel::Error,
            message: "".to_string(),
        }
    }
}