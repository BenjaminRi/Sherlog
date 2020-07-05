extern crate chrono;

use chrono::prelude::*;
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

pub struct LogEntry {
	pub timestamp: chrono::DateTime<Utc>,
	pub severity: LogLevel,
	pub message: String,
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
			timestamp: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
			severity: LogLevel::Error,
			message: "".to_string(),
		}
	}
}
