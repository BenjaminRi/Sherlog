use super::super::model;

extern crate chrono;

use std::io::BufReader;
use std::io::BufRead;
use std::io::Read;
use chrono::prelude::*;

// GLOG parser ----------------------------------------------------------------------

pub fn to_log_entries(reader: impl std::io::Read) -> Vec::<model::LogEntry> {
	let mut log_entries = Vec::<model::LogEntry>::new();
	
	let mut parser = GlogParserState::PreSection;
	let mut parser_str = Vec::with_capacity(512);
	let mut log_entry = model::LogEntry { ..Default::default() };
	
	let mut bufreader = BufReader::new(reader);
	let mut buffer = [0; 1];
    while let Ok(bytes_read) = bufreader.read(&mut buffer){
		if(bytes_read == 0) {
			break;
		}
		let chr = buffer[0];
		//println!("Char: {}", chr as i32);
		
		parser = match parser {
			GlogParserState::PreSection => {
				if chr == b'[' {
					GlogParserState::SectionKind
				} else if chr == b'\r' || chr == b'\n' {
					GlogParserState::PreSection
				} else {
					println!("Invalid symbol encountered before log line: {}", chr);
					GlogParserState::PreSection
				}
			},
			GlogParserState::SectionKind => {
				if chr == b'|' {
					let kind_str = std::str::from_utf8(&parser_str);
					//println!("{}", &String::from_utf8_lossy(&parser_str));
					
					let kind = if let Ok(kind_str) = kind_str {
						match kind_str.as_ref() {
							"tq" => GlogSectionKind::TimestampMs,
							"s" => GlogSectionKind::Severity,
							"i" => GlogSectionKind::LogSource,
							"m" => GlogSectionKind::Message,
							_ => GlogSectionKind::Unknown, //TODO: Notify of invalid sections?
						}
					} else {
						GlogSectionKind::Unknown //TODO: Notify of malformed UTF-8?
					};
					parser_str.clear();
					GlogParserState::SectionValue(kind)
				} else {
					parser_str.push(chr);
					GlogParserState::SectionKind
				}
			},
			GlogParserState::SectionValue(kind) => {
				parser_str.push(chr);
				if chr == b']' {
					GlogParserState::SectionValuePost1(kind)
				} else {
					GlogParserState::SectionValue(kind)
				}
			},
			GlogParserState::SectionValuePost1(kind) => {
				parser_str.push(chr);
				if chr == b':' {
					GlogParserState::SectionValuePost3(kind, 3, false)
				} else if chr == b'\r' {
					GlogParserState::SectionValuePost2(kind)
				} else if chr == b'\n' {
					GlogParserState::SectionValuePost3(kind, 3, true)
				} else {
					GlogParserState::SectionValue(kind)
				}
			},
			GlogParserState::SectionValuePost2(kind) => {
				parser_str.push(chr);
				if chr == b'\n' {
					GlogParserState::SectionValuePost3(kind, 4, true)
				} else {
					GlogParserState::SectionValue(kind)
				}
			},
			GlogParserState::SectionValuePost3(kind, suffix_cutoff, entry_done) => {
				parser_str.push(chr);
				if chr == b'[' {
					let value_str = String::from_utf8_lossy(&parser_str[0..parser_str.len()-suffix_cutoff]);
					
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
					if(entry_done) {
						log_entries.push(log_entry);
						log_entry = model::LogEntry { ..Default::default() };
					}
					parser_str.clear();
					GlogParserState::SectionKind
				} else {
					GlogParserState::SectionValue(kind)
				}
			},
		};
	}
	
	log_entries
}

enum GlogParserState {
	PreSection,                               //expect '[', ignore '\r' or '\n'
	SectionKind,                       //expect kind until '|' (kind may not contain '|')
	SectionValue(GlogSectionKind),     //expect value until ']'
	SectionValuePost1(GlogSectionKind), //expect ':' or '\r' or '\n', process line
	SectionValuePost2(GlogSectionKind),
	SectionValuePost3(GlogSectionKind, usize, bool),
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
