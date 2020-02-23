extern crate chrono;

use chrono::prelude::*;

use crate::model;

pub struct LogEntryExt {
	pub timestamp : chrono::DateTime<Utc>,
	pub severity : model::LogLevel,
	pub message : String,
	pub source_id : u32,
	pub visible : bool,
	pub entry_id : u32,
}

// Extended log source (not part of the API)
pub enum LogSourceContentsExt {
	Sources(Vec::<LogSourceExt>),
	Entries(Vec::<LogEntryExt>),
}

// Extended log source (not part of the API)
pub struct LogSourceExt {
	pub name : String,
	pub id : u32,
	pub child_cnt : u64,
	pub children : LogSourceContentsExt,
}