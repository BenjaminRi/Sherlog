extern crate chrono;

use chrono::prelude::*;

use crate::model;

pub const VISIBLE_ON: u8 = 0x0;
pub const VISIBLE_OFF_SOURCE: u8 = 0x1;
pub const VISIBLE_OFF_SEVERITY: u8 = 0x2;
pub const VISIBLE_OFF_FILTER: u8 = 0x4;

pub struct LogEntryExt {
	pub timestamp : chrono::DateTime<Utc>,
	pub severity : model::LogLevel,
	pub message : String,
	pub source_id : u32,
	pub visible : u8,
	pub entry_id : u32,
	pub prev_offset : u32,
	pub next_offset : u32,
	//For 1 million objects, 3 uint32 require 20ms more to sort.
	//Therefore, stick to uint32 and not usize which doubles this amount
}

impl LogEntryExt {
	pub fn is_visible(& self) -> bool {
		self.visible == VISIBLE_ON
	}
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

impl LogSourceExt {
	pub fn from_source(log_source : model::LogSource) -> LogSourceExt {
		let children = match log_source.children {
			model::LogSourceContents::Sources(v) => {
				let mut contents = Vec::<LogSourceExt>::new();
				contents.reserve(v.len());
				for source in v {
					contents.push(LogSourceExt::from_source(source));
				}
				LogSourceContentsExt::Sources(contents)
			},
			model::LogSourceContents::Entries(v) => {
				LogSourceContentsExt::Entries(
					v.into_iter().map(
						move |entry| LogEntryExt {
							timestamp: entry.timestamp,
							severity: entry.severity,
							message: entry.message,
							source_id: 0,
							visible: VISIBLE_ON,
							entry_id : 0,
							prev_offset : 0,
							next_offset : 0,
						}
					).collect())
			},
		};
		let mut source_ext = LogSourceExt {name: log_source.name, id: 0, child_cnt: 0, children: children};
		source_ext.generate_ids();
		source_ext.calc_child_cnt();
		source_ext
	}
	
	fn calc_child_cnt(&mut self) {
		self.child_cnt = match &mut self.children {
			LogSourceContentsExt::Sources(v) => {
				let mut child_cnt : u64 = 0;
				for source in v {
					source.calc_child_cnt();
					child_cnt += source.child_cnt;
				}
				child_cnt
			},
			LogSourceContentsExt::Entries(v) => {
				v.len() as u64
			},
		}
	}
	fn generate_ids(&mut self) -> u32 {
		match &mut self.children {
			LogSourceContentsExt::Sources(v) => {
				let mut id_idx = self.id;
				for source in v {
					id_idx += 1;
					source.id = id_idx;
					id_idx = source.generate_ids();
				}
				id_idx
			},
			LogSourceContentsExt::Entries(v) => {
				for entry in v.iter_mut() {
					entry.source_id = self.id;
				}
				self.id
			},
		}
	}
}
