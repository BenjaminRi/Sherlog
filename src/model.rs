extern crate chrono;

use chrono::prelude::*;
use std::collections::HashMap;
use std::fmt;

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

impl fmt::Display for LogLevel {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(
			f,
			"{}",
			match self {
				LogLevel::Critical => "Critical",
				LogLevel::Error => "Error",
				LogLevel::Warning => "Warning",
				LogLevel::Info => "Info",
				LogLevel::Debug => "Debug",
				LogLevel::Trace => "Trace",
			}
		)
	}
}

#[allow(dead_code)]
pub enum CustomField {
	Int64(i64),
	Int32(i32),
	Int16(i16),
	Int8(i8),
	UInt64(u64),
	UInt32(u32),
	UInt16(u16),
	UInt8(u8),
	Float32(f32),
	Float64(f64),
	String(String),
}

pub struct LogEntry {
	pub timestamp: chrono::DateTime<Utc>,
	pub severity: LogLevel,
	pub message: String,
	pub custom_fields: HashMap<std::borrow::Cow<'static, str>, CustomField>,
}

pub enum LogSourceContents {
	Sources(Vec<LogSource>),
	Entries(Vec<LogEntry>),
}

pub struct LogSource {
	pub name: String,
	pub children: LogSourceContents,
}

impl Default for LogEntry {
	fn default() -> LogEntry {
		LogEntry {
			timestamp: DateTime::<Utc>::from_utc(
				NaiveDateTime::from_timestamp_opt(0, 0).unwrap(),
				Utc,
			),
			severity: LogLevel::Error,
			message: "".to_string(),
			custom_fields: HashMap::new(),
		}
	}
}
