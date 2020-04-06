use super::super::model;

extern crate chrono;

use chrono::prelude::*;
use std::collections::HashMap;
use std::io::BufReader;
use std::io::Read;
use std::mem;

// GLOG parser ----------------------------------------------------------------------

pub fn to_log_entries(reader: impl std::io::Read, root: model::LogSource) -> model::LogSource {
	let mut parser = GlogParser::new(root);

	let mut bufreader = BufReader::new(reader);
	let mut buffer = [0; 1];
	loop {
		if let Ok(bytes_read) = bufreader.read(&mut buffer) {
			if bytes_read == 0 {
				//println!("Len srcs {}, entrs {}", parser.log_sources.len(), parser.log_entries.len());
				break parser.finalize();
			} else {
				parser.read_byte(buffer[0]);
			}
		} else {
			break parser.finalize();
		}
	}
}

struct GlogParser {
	state: GlogParserState,
	buf: Vec<u8>,
	log_entry: model::LogEntry,
	sub_source: Option<i32>,
	log_entries: Vec<model::LogEntry>,
	log_sources: HashMap<i32, model::LogSource>,
	invalid_bytes: usize,
	root: model::LogSource,
}

impl GlogParser {
	fn new(root: model::LogSource) -> GlogParser {
		GlogParser {
			state: GlogParserState::PreSection,
			buf: Vec::with_capacity(512),
			log_entry: model::LogEntry {
				..Default::default()
			},
			sub_source: None,
			log_entries: Vec::<model::LogEntry>::new(),
			log_sources: HashMap::<i32, model::LogSource>::new(),
			invalid_bytes: 0,
			root: root,
		}
	}

	fn read_byte(&mut self, chr: u8) {
		self.state = match self.state {
			GlogParserState::PreSection => {
				if chr == b'[' {
					GlogParserState::SectionKind
				} else if chr == b'\r' || chr == b'\n' {
					GlogParserState::PreSection
				} else {
					self.invalid_bytes += 1;
					GlogParserState::PreSection
				}
			}
			GlogParserState::SectionKind => {
				if chr == b'|' {
					let kind_str = std::str::from_utf8(&self.buf);

					let kind = if let Ok(kind_str) = kind_str {
						match kind_str.as_ref() {
							"tq" => GlogSectionKind::TimestampMs, //controller only
							"s" => GlogSectionKind::Severity,
							"i" => GlogSectionKind::LogSource, //controller only
							"m" => GlogSectionKind::Message,
							"e" => GlogSectionKind::ErrorCode, //sensor only
							"n" => GlogSectionKind::SessionId, //sensor only
							"t" => GlogSectionKind::Timestamp100ns, //sensor only
							_ => {
								//TODO: Notify of invalid kind?
								println!("UNRECOGNIZED kind: {}", &kind_str);
								GlogSectionKind::Unknown
							}
						}
					} else {
						//TODO: Notify of malformed UTF-8?
						println!(
							"MALFORMED UTF-8 in kind string: {}",
							&String::from_utf8_lossy(&self.buf)
						);
						GlogSectionKind::Unknown
					};
					self.buf.clear();
					GlogParserState::SectionValue(kind)
				} else {
					self.buf.push(chr);
					GlogParserState::SectionKind
				}
			}
			GlogParserState::SectionValue(kind) => {
				self.buf.push(chr);
				if chr == b']' {
					GlogParserState::SectionValuePost1(kind)
				} else {
					GlogParserState::SectionValue(kind)
				}
			}
			GlogParserState::SectionValuePost1(kind) => {
				self.buf.push(chr);
				if chr == b':' {
					GlogParserState::SectionValuePost3(kind, 3, false)
				} else if chr == b'\r' {
					GlogParserState::SectionValuePost2(kind)
				} else if chr == b'\n' {
					GlogParserState::SectionValuePost3(kind, 3, true)
				} else if chr == b']' {
					GlogParserState::SectionValuePost1(kind)
				} else {
					GlogParserState::SectionValue(kind)
				}
			}
			GlogParserState::SectionValuePost2(kind) => {
				self.buf.push(chr);
				if chr == b'\n' {
					GlogParserState::SectionValuePost3(kind, 4, true)
				} else if chr == b']' {
					GlogParserState::SectionValuePost1(kind)
				} else {
					GlogParserState::SectionValue(kind)
				}
			}
			GlogParserState::SectionValuePost3(kind, suffix_cutoff, entry_done) => {
				self.buf.push(chr);
				if chr == b'[' {
					let value_str =
						String::from_utf8_lossy(&self.buf[0..self.buf.len() - suffix_cutoff]);

					match kind {
						GlogSectionKind::TimestampMs => {
							if let Ok(ts_milli) = value_str.parse::<u64>() {
								let ts_sec: u64 = ts_milli / 1000;
								let ts_nano: u32 = ((ts_milli - ts_sec * 1000) * 1_000_000) as u32;
								if let Some(ndt) =
									NaiveDateTime::from_timestamp_opt(ts_sec as i64, ts_nano)
								{
									self.log_entry.timestamp = DateTime::<Utc>::from_utc(ndt, Utc);
								} else {
									//TODO: Notify of invalid datetime?
									println!("MALFORMED Log datetime: {}", value_str);
								}
							} else {
								//TODO: Notify of invalid timestamp?
								println!("MALFORMED Log timestamp: {}", value_str);
							}
						}
						GlogSectionKind::Severity => {
							if let Ok(glog_sev_u32) = value_str.parse::<u32>() {
								if let Some(glog_sev) = GlogSeverity::from_u32(glog_sev_u32) {
									self.log_entry.severity = normalize_glog_sev(glog_sev);
								} else {
									//TODO: Notify of invalid severity?
									println!("INVALID Log severity: {}", value_str);
								}
							} else {
								//TODO: Notify of malformed severity?
								println!("MALFORMED Log severity: {}", value_str);
							}
						}
						GlogSectionKind::LogSource => {
							if let Ok(parsed_sub_source) = value_str.parse::<i32>() {
								self.sub_source = Some(parsed_sub_source);
							} else {
								//TODO: Notify of malformed sub-source?
								println!("MALFORMED Log sub-source: {}", value_str);
							}
						}
						GlogSectionKind::Message => {
							if let std::borrow::Cow::Owned(owned_str) = &value_str {
								println!("MALFORMED UTF-8 in Message: {}", owned_str);
							}
							self.log_entry.message = value_str.to_string();
						}
						GlogSectionKind::Timestamp100ns => {
							if let Ok(mut ts_100ns) = value_str.parse::<u64>() {
								// 100-nanosecond offset from 0000-01-01 00:00:00.000 to 1970-01-01 00:00:00.000
								const TIME_OFFSET: u64 = 621_355_968_000_000_000;
								if ts_100ns >= TIME_OFFSET {
									ts_100ns -= TIME_OFFSET;
									let ts_sec: u64 = ts_100ns / 10_000_000;
									let ts_nano: u32 = (ts_100ns - ts_sec * 10_000_000) as u32;
									if let Some(ndt) =
										NaiveDateTime::from_timestamp_opt(ts_sec as i64, ts_nano)
									{
										self.log_entry.timestamp = DateTime::<Utc>::from_utc(ndt, Utc);
									} else {
										//TODO: Notify of invalid datetime?
										println!("MALFORMED Log 100ns stamp datetime: {}", value_str);
									}
								} else {
									//TODO: Notify of invalid offset?
									println!("MALFORMED Log 100ns stamp offset: {}", ts_100ns);
								}
							} else {
								//TODO: Notify of invalid time?
								println!("MALFORMED Log 100ns stamp: {}", value_str);
							}
						}
						GlogSectionKind::ErrorCode => {
							//TODO: Expose ErrorCode to user
						}
						GlogSectionKind::SessionId => {
							//TODO: Handle session ID, in particular sorting
							//with session ID instead of timestamp
						}
						GlogSectionKind::Unknown => (),
					}
					if entry_done {
						let log_entry = mem::replace(
							&mut self.log_entry,
							model::LogEntry {
								..Default::default()
							},
						);
						if let Some(sub_source) = self.sub_source {
							//Log entry specified a log sub-source
							let source_option = self.log_sources.get_mut(&sub_source);
							if let Some(source) = source_option {
								//Log sub-source exists, push log entry
								let children = &mut source.children;
								match children {
									model::LogSourceContents::Entries(v) => {
										v.push(log_entry);
									}
									_ => (),
								}
							} else {
								//Log sub-source does not yet exist
								self.log_sources.insert(
									sub_source,
									model::LogSource {
										name: sub_source.to_string(),
										children: {
											model::LogSourceContents::Entries(vec![log_entry])
										},
									},
								);
							}
						} else {
							//Log entry did not specify a log sub-source
							self.log_entries.push(log_entry);
						}
						self.sub_source = None;
					}
					self.buf.clear();
					GlogParserState::SectionKind
				} else if chr == b']' {
					GlogParserState::SectionValuePost1(kind)
				} else {
					GlogParserState::SectionValue(kind)
				}
			}
		};
	}

	fn finalize(mut self) -> model::LogSource {
		if self.invalid_bytes > 0 {
			//TODO: Invalid bytes?
			println!("INVALID bytes encountered, count: {}", self.invalid_bytes);
		}
		match self.state {
			GlogParserState::PreSection => {
				//Log file empty
			}
			GlogParserState::SectionKind => {
				//TODO: Notify of cut off kind?
				println!("CUT OFF last log message (kind)");
			}
			GlogParserState::SectionValue(_) => {
				//TODO: Notify of cut off kind?
				println!("CUT OFF last log message (value)");
			}
			GlogParserState::SectionValuePost1(_) => {
				//Finish parsing section
				self.read_byte(b'\n');
				self.read_byte(b'[');
			}
			GlogParserState::SectionValuePost2(_) => {
				//Finish parsing section
				self.read_byte(b'\n');
				self.read_byte(b'[');
			}
			GlogParserState::SectionValuePost3(_, _, _) => {
				//Finish parsing section
				self.read_byte(b'[');
			}
		};

		if self.log_sources.is_empty() {
			self.root.children = model::LogSourceContents::Entries(self.log_entries);
		} else {
			let mut v = Vec::<model::LogSource>::with_capacity(self.log_sources.len());
			for (_, mut sub_source) in self.log_sources {
				sub_source.name = match &sub_source.children {
					model::LogSourceContents::Entries(v) => {
						if !v.is_empty() {
							v[0].message.split(":").nth(0).unwrap().to_string() //TODO: Handle error
						} else {
							"???".to_string() //TODO: Handle error
						}
					}
					_ => "???".to_string(), //TODO: Handle error
				};
				v.push(sub_source);
			}
			self.root.children = model::LogSourceContents::Sources(v);
		}

		self.root
	}
}

#[derive(Copy, Clone)]
enum GlogParserState {
	PreSection,                                      //expect '[', ignore '\r' or '\n'
	SectionKind,                                     //expect kind until '|' (kind may not contain '|')
	SectionValue(GlogSectionKind),                   //expect value until ']'
	SectionValuePost1(GlogSectionKind),              //expect ':' or '\r' or '\n'
	SectionValuePost2(GlogSectionKind),              //expect '\n'
	SectionValuePost3(GlogSectionKind, usize, bool), //expect ']', process line
}

fn normalize_glog_sev(glog_sev: GlogSeverity) -> model::LogLevel {
	return match glog_sev {
		GlogSeverity::Critical => model::LogLevel::Critical,
		GlogSeverity::Hardware => model::LogLevel::Critical,
		GlogSeverity::Error => model::LogLevel::Error,
		GlogSeverity::Warning => model::LogLevel::Warning,
		GlogSeverity::Info => model::LogLevel::Info,
		GlogSeverity::None => model::LogLevel::Critical,
	};
}

enum GlogSeverity {
	Critical = 0,
	Hardware = 1,
	Error = 2,
	Warning = 3,
	Info = 4,
	None = 5,
}

impl GlogSeverity {
	fn from_u32(value: u32) -> Option<GlogSeverity> {
		match value {
			0 => Some(GlogSeverity::Critical),
			1 => Some(GlogSeverity::Hardware),
			2 => Some(GlogSeverity::Error),
			3 => Some(GlogSeverity::Warning),
			4 => Some(GlogSeverity::Info),
			5 => Some(GlogSeverity::None),
			_ => None,
		}
	}
}

#[derive(Debug, PartialEq, Copy, Clone)]
enum GlogSectionKind {
	TimestampMs,
	Severity,
	LogSource,
	Message,
	Timestamp100ns,
	ErrorCode,
	SessionId,
	Unknown,
}
