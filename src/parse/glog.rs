use super::super::model;

use super::datetime_utils;

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
				//log::info!("Len srcs {}, entrs {}", parser.log_sources.len(), parser.log_entries.len());
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
	log_sources: HashMap<String, model::LogSource>,
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
			log_sources: HashMap::<String, model::LogSource>::new(),
			invalid_bytes: 0,
			root,
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
						match kind_str {
							"tq" => GlogSectionKind::TimestampMs, //controller only
							"s" => GlogSectionKind::Severity,
							"i" => GlogSectionKind::LogSource, //controller only
							"m" => GlogSectionKind::Message,
							"e" => GlogSectionKind::ErrorCode,      //sensor only
							"n" => GlogSectionKind::SessionId,      //sensor only
							"t" => GlogSectionKind::Timestamp100ns, //sensor only
							_ => {
								//TODO: Notify of invalid kind?
								log::warn!("UNRECOGNIZED kind: {}", &kind_str);
								GlogSectionKind::Unknown
							}
						}
					} else {
						//TODO: Notify of malformed UTF-8?
						log::warn!(
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
								if let Some(datetime) = datetime_utils::from_timestamp_ms(ts_milli)
								{
									self.log_entry.timestamp = datetime;
								} else {
									//TODO: Notify of invalid datetime?
									log::warn!("MALFORMED Log ms timestamp: {}", ts_milli);
								}
							} else {
								//TODO: Notify of invalid timestamp?
								log::warn!("MALFORMED Log ms timestamp value: {}", value_str);
							}
						}
						GlogSectionKind::Severity => {
							if let Ok(glog_sev_u32) = value_str.parse::<u32>() {
								if let Some(glog_sev) = GlogSeverity::from_u32(glog_sev_u32) {
									self.log_entry.severity = normalize_glog_sev(glog_sev);
								} else {
									//TODO: Notify of invalid severity?
									log::warn!("INVALID Log severity: {}", value_str);
								}
							} else {
								//TODO: Notify of malformed severity?
								log::warn!("MALFORMED Log severity: {}", value_str);
							}
						}
						GlogSectionKind::LogSource => {
							if let Ok(parsed_sub_source) = value_str.parse::<i32>() {
								self.sub_source = Some(parsed_sub_source);
							//self.log_entry.custom_fields.insert(
							//	std::borrow::Cow::Borrowed("LogSource"),
							//	model::CustomField::Int32(parsed_sub_source),
							//);
							} else {
								//TODO: Notify of malformed sub-source?
								log::warn!("MALFORMED Log sub-source: {}", value_str);
							}
						}
						GlogSectionKind::Message => {
							if let std::borrow::Cow::Owned(owned_str) = &value_str {
								log::warn!("MALFORMED UTF-8 in Message: {}", owned_str);
							}
							self.log_entry.message = value_str.to_string();
						}
						GlogSectionKind::Timestamp100ns => {
							if let Ok(gcom_datetime) = value_str.parse::<u64>() {
								if let Some(datetime) = datetime_utils::from_100ns(gcom_datetime) {
									self.log_entry.timestamp = datetime;
								} else {
									//TODO: Notify of invalid datetime?
									log::warn!("MALFORMED Log 100ns datetime: {}", gcom_datetime);
								}
							} else {
								//TODO: Notify of invalid 100ns value?
								log::warn!("MALFORMED Log 100ns value: {}", value_str);
							}
						}
						GlogSectionKind::ErrorCode => {
							//TODO: Expose ErrorCode to user
						}
						GlogSectionKind::SessionId => {
							//TODO: Handle session ID, in particular sorting
							//with session ID instead of timestamp
							if let Ok(parsed_session_id) = value_str.parse::<u32>() {
								self.log_entry.custom_fields.insert(
									std::borrow::Cow::Borrowed("SessionId"),
									model::CustomField::UInt32(parsed_session_id),
								);
							} else {
								//TODO: Notify of malformed session id?
								log::warn!("MALFORMED Session ID: {}", value_str);
							}
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

							//Note: We don't actually use QNX log source IDs properly
							//All we do is prepend source string name, plus colon and space

							let offset = log_entry.message.find(": ");
							let source_name = if let Some(offset) = offset {
								let name_slice = &log_entry.message[0..offset];
								if !name_slice.is_empty() {
									Some(std::borrow::Cow::from(name_slice))
								} else {
									None
								}
							} else {
								None
							}
							.unwrap_or_else(|| {
								std::borrow::Cow::from(format!("Unknown ({})", sub_source))
							});

							let source_option = self.log_sources.get_mut(source_name.as_ref());
							if let Some(source) = source_option {
								//Log sub-source exists, push log entry
								let children = &mut source.children;
								match children {
									model::LogSourceContents::Entries(v) => {
										v.push(log_entry);
									}
									_ => unreachable!(), //We only insert LogSourceContents::Entries
								}
							} else {
								//Log sub-source does not yet exist
								self.log_sources.insert(
									source_name.to_string(),
									model::LogSource {
										name: source_name.to_string(),
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
			log::warn!("INVALID bytes encountered, count: {}", self.invalid_bytes);
		}
		match self.state {
			GlogParserState::PreSection => {
				//Log file empty
			}
			GlogParserState::SectionKind => {
				//TODO: Notify of cut off kind?
				log::warn!("CUT OFF last log message (kind)");
			}
			GlogParserState::SectionValue(_) => {
				//TODO: Notify of cut off kind?
				log::warn!("CUT OFF last log message (value)");
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
			//If no log message specified a source, we put the entries directly into the root
			self.root.children = model::LogSourceContents::Entries(self.log_entries);
		} else {
			let mut v = Vec::<model::LogSource>::with_capacity(
				self.log_sources.len() + !self.log_entries.is_empty() as usize,
			);
			for (_, sub_source) in self.log_sources {
				v.push(sub_source);
			}

			if !self.log_entries.is_empty() {
				let sub_source = model::LogSource {
					name: "Unknown (None)".to_string(),
					children: { model::LogSourceContents::Entries(self.log_entries) },
				};
				v.push(sub_source);
			}

			//Case insensitive sort by log source name
			v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
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
	match glog_sev {
		GlogSeverity::Critical => model::LogLevel::Critical,
		GlogSeverity::Hardware => model::LogLevel::Critical,
		GlogSeverity::Error => model::LogLevel::Error,
		GlogSeverity::Warning => model::LogLevel::Warning,
		GlogSeverity::Info => model::LogLevel::Info,
		GlogSeverity::None => model::LogLevel::Debug,
		//Note: We map GlogSeverity::None to Debug.
		//GlogSeverity::None is often used to dump unstructured
		//log output whose severity is not known. Classifying that
		//as critical would be misleading, as it creates the
		//impression that there is a large number of critical errors.
		//A log message with this severity could be anything, from
		//critical error all the way to verbose debug data!!
		//
		//In my opinion, GlogSeverity::None shouldn't even exist,
		//as it allows unstructured log data inside a structured log
		//and delegates the difficult problem of log classification
		//to the outside world which knows even less about the true
		//severity of the messages. Therefore, firmware that logs with
		//GlogSeverity::None shall be considered buggy and should be fixed.
		//The usage of the GLOG severity SEV_NONE in the firmware's
		//C++ code shall be replaced with the appropriate severity!
		//Only sensor firmware contains this bug, controller firmware
		//does it right by virtue of using QNX's slogger/slogger2.
	}
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
