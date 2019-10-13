use super::super::model;

extern crate chrono;

use chrono::prelude::*;

// GLOG parser ----------------------------------------------------------------------

pub fn to_log_entries(mut reader: impl std::io::BufRead) -> Vec::<model::LogEntry> {
	let mut log_entries = Vec::<model::LogEntry>::new();
	let mut buf = Vec::<u8>::new();
    while reader.read_until(b'\n', &mut buf).expect("read_until failed") != 0 {
		match String::from_utf8_lossy(&buf) {
			std::borrow::Cow::Borrowed(line_str) => {
				//println!("{}", line_str);
				let curr_entry = line_to_log_entry(&line_str);
				log_entries.push(curr_entry);
				buf.clear();
			},
			std::borrow::Cow::Owned(line_str) => {
				line_to_log_entry(&line_str);
				//TODO: Notify of invalid lines?
				println!("MALFORMED UTF-8: {}", line_str);
			},
		}
    }
	log_entries
}

pub fn line_to_log_entry(line: &str) -> model::LogEntry {
	let mut log_entry = model::LogEntry { ..Default::default() };
	let mut parser = GlogParserState::PreSection;
	for idx in line.char_indices() {
		parser = match parser {
			GlogParserState::PreSection => {
				if idx.1 == GLOG_SECTION_BEGIN {
					GlogParserState::SectionKind(idx.0 + GLOG_SECTION_BEGIN_SZ)
				} else if idx.1 == GLOG_NEWLINE_R || idx.1 == GLOG_NEWLINE_N {
					GlogParserState::PreSection
				} else {
					GlogParserState::Invalid
				}
			},
			GlogParserState::SectionKind(kind_offset) => {
				if idx.1 == GLOG_INNER_DELIM {
					let kind_str = &line[kind_offset..idx.0];
					//println!("{}", &kind_str);
					let kind = match kind_str {
						"tq" => GlogSectionKind::TimestampMs,
						"s" => GlogSectionKind::Severity,
						"i" => GlogSectionKind::LogSource,
						"m" => GlogSectionKind::Message,
						_ => GlogSectionKind::Unknown, //TODO: Notify of invalid sections?
					};
					GlogParserState::SectionValue(kind, idx.0 + GLOG_INNER_DELIM_SZ)
				} else {
					GlogParserState::SectionKind(kind_offset)
				}
			},
			GlogParserState::SectionValue(kind, value_offset) => {
				if idx.1 == GLOG_SECTION_END {
					GlogParserState::SectionValuePost(kind, value_offset)
				} else {
					GlogParserState::SectionValue(kind, value_offset)
				}
			},
			GlogParserState::SectionValuePost(kind, value_offset) => {
				if idx.1 == GLOG_SECTION_DELIM || idx.1 == GLOG_NEWLINE_R || idx.1 == GLOG_NEWLINE_N {
					if idx.1 == GLOG_SECTION_DELIM && kind == GlogSectionKind::Message {
						//Message is always last - ignore "]:"
						GlogParserState::SectionValue(kind, value_offset)
					} else {
						//Add field to log entry
						let value_str = &line[value_offset..idx.0-GLOG_SECTION_END_SZ];
						//println!("Kind: {:?}, Value: {}", kind, value_str);
						match kind {
							GlogSectionKind::TimestampMs => {
								if let Ok(ts_milli) = value_str.parse::<u64>() {
									let ts_sec   : u64 = ts_milli / 1000;
									let ts_nano  : u32 = ((ts_milli - ts_sec * 1000) * 1000_000) as u32;
									if let Some(ndt) = NaiveDateTime::from_timestamp_opt(ts_sec as i64, ts_nano) {
										log_entry.timestamp = DateTime::<Utc>::from_utc(ndt, Utc);
									} else {
										//TODO: Notify of invalid datetime?
										println!("MALFORMED Log datetime: {}", value_str);
									}
								} else {
									//TODO: Notify of invalid timestamp?
									println!("MALFORMED Log timestamp: {}", value_str);
								}
							},
							GlogSectionKind::Severity => {
								if let Ok(glog_sev_u32) = value_str.parse::<u32>() {
									if let Some(glog_sev) = GlogSeverity::from_u32(glog_sev_u32) {
										log_entry.severity = normalize_glog_sev(glog_sev);
									}
								}
							},
							GlogSectionKind::Message => {
								log_entry.message = value_str.to_string();
							},
							_ => (),
						}
						GlogParserState::PreSection
					}
				} else if idx.1 == GLOG_SECTION_END {
					GlogParserState::SectionValuePost(kind, value_offset)
				} else {
					GlogParserState::SectionValue(kind, value_offset)
				}
			},
			GlogParserState::Invalid => {
				//TODO: Notify of invalid lines?
				println!("MALFORMED Log line: {}", line);
				break;
			},
		};
	}
	log_entry
}

fn normalize_glog_sev(glog_sev : GlogSeverity) -> model::LogLevel {
	return match glog_sev {
		GlogSeverity::Critical => model::LogLevel::Critical,
		GlogSeverity::Hardware => model::LogLevel::Critical,
		GlogSeverity::Error    => model::LogLevel::Error,
		GlogSeverity::Warning  => model::LogLevel::Warning,
		GlogSeverity::Info     => model::LogLevel::Info,
		GlogSeverity::None     => model::LogLevel::Critical,
	}
}

const GLOG_SECTION_BEGIN : char = '[';
const GLOG_SECTION_BEGIN_SZ : usize = 1; //GLOG_SECTION_BEGIN.len_utf8();
const GLOG_INNER_DELIM : char = '|';
const GLOG_INNER_DELIM_SZ : usize = 1; //GLOG_INNER_DELIM.len_utf8();
const GLOG_SECTION_END : char = ']';
const GLOG_NEWLINE_R : char = '\r';
const GLOG_NEWLINE_N : char = '\n';
const GLOG_SECTION_END_SZ : usize = 1; //GLOG_SECTION_END.len_utf8();
const GLOG_SECTION_DELIM : char = ':';
//const GLOG_SECTION_DELIM_SZ : usize = 1; //GLOG_SECTION_DELIM.len_utf8();

enum GlogSeverity
{
	Critical = 0,
	Hardware = 1,
	Error    = 2,
	Warning  = 3,
	Info     = 4,
	None     = 5,
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
			_ => None
		}
	}
}

#[derive(Debug, PartialEq)]
enum GlogSectionKind {
	TimestampMs,
	Severity,
	LogSource,
	Message,
	Unknown,
}

enum GlogParserState {
	PreSection,                               //expect '[', ignore '\r' or '\n'
	SectionKind(usize),                       //expect kind until '|' (kind may not contain '|')
	SectionValue(GlogSectionKind, usize),     //expect value until ']'
	SectionValuePost(GlogSectionKind, usize), //expect ':' or '\r' or '\n', process line
	Invalid,                                  //park in this state on parser error
}

// -------------------------------------------------------------------------------------------------------------