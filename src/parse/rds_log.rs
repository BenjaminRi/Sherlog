use super::super::model;

extern crate chrono;
use chrono::{DateTime, NaiveDateTime, Utc};

use std::io::BufRead;
use std::io::BufReader;

use std::io::Result;

pub fn parse_rds_datetime(dt_string: &str) -> Option<chrono::DateTime<Utc>> {
	// TODO: What time zone does RDS log with? UTC? Local time?
	// If it's local time, we will never know the actual time because the logs
	// do not contain any information about location and time zone.
	// For now, assume UTC and hope for the best...
	if let Ok(ndt) = NaiveDateTime::parse_from_str(dt_string, "%Y-%m-%d %H:%M:%S%.f") {
		Some(DateTime::<Utc>::from_utc(ndt, Utc))
	} else {
		None
	}
}

fn read_until_pipe_or_newline(mut reader: impl std::io::Read, buf: &mut Vec<u8>) -> Result<usize> {
	let mut bytes_read = 0;
	let mut byte = [0; 1];
	loop {
		match reader.read(&mut byte) {
			Ok(0) => return Ok(bytes_read),
			Err(e) => return Err(e),
			Ok(_) => {
				bytes_read += 1;
				let byte = byte[0];
				buf.push(byte);
				if byte == b'|' || byte == b'\n' {
					return Ok(bytes_read);
				}
			}
		}
	}
}

pub fn to_log_entries(reader: impl std::io::Read, mut root: model::LogSource) -> model::LogSource {
	let mut log_entries = Vec::<model::LogEntry>::new();
	let mut bufreader = BufReader::new(reader);
	let mut buf = Vec::<u8>::with_capacity(512);
	let mut parser_state = RdsLogParserState::ExpectDatetime;

	let mut log_entry = model::LogEntry {
		..Default::default()
	};

	loop {
		match parser_state {
			RdsLogParserState::ExpectDatetime => {
				if let Ok(bytes_read) = bufreader.read_until(b'|', &mut buf) {
					if bytes_read != 0 {
						if buf.last() == Some(&b'|') {
							buf.pop();
							let buf_str = String::from_utf8_lossy(&buf);
							if let std::borrow::Cow::Owned(owned_str) = &buf_str {
								println!("MALFORMED UTF-8 in datetime: {}", owned_str);
							} else {
								if let Some(timestamp) = parse_rds_datetime(&buf_str) {
									log_entry.timestamp = timestamp;
								} else {
									//TODO: Notify of invalid datetime?
									println!("MALFORMED Log datetime: {}", buf_str);
								}
							}
							buf.clear();
							parser_state = RdsLogParserState::ExpectErrcodeOrSeverity;
						} else {
							//TODO: Error, log file ended before delimiter was found.
							break;
						}
					} else {
						//Log file is empty. Nothing to do.
						break;
					}
				} else {
					//TODO: Read error
					break;
				}
			}
			RdsLogParserState::ExpectErrcodeOrSeverity => {
				if let Ok(bytes_read) = bufreader.read_until(b'|', &mut buf) {
					if bytes_read != 0 {
						if buf.last() == Some(&b'|') {
							buf.pop();
							let buf_str = String::from_utf8_lossy(&buf);
							if let std::borrow::Cow::Owned(owned_str) = &buf_str {
								println!("MALFORMED UTF-8 in error code / severity: {}", owned_str);
								parser_state = RdsLogParserState::ExpectSeverity;
							} else if let Ok(_error_code) = buf_str.trim_start().parse::<u32>() {
								//Successfully read error code; discard it for now
								parser_state = RdsLogParserState::ExpectSeverity;
							} else if let Some(rds_log_sev) = RdsLogSeverity::from_str(&buf_str) {
								log_entry.severity = normalize_rds_log_sev(rds_log_sev);
								parser_state = RdsLogParserState::ExpectLogSource;
							} else {
								//TODO: Notify of invalid error code / severity?
								println!("MALFORMED error code / severity: {}", buf_str);
								parser_state = RdsLogParserState::ExpectSeverity;
							}
							buf.clear();
						} else {
							//TODO: Error, log file ended before delimiter was found.
							break;
						}
					} else {
						//TODO: Log file cut off... Error.
						break;
					}
				} else {
					//TODO: Read error
					break;
				}
			}
			RdsLogParserState::ExpectSeverity => {
				if let Ok(bytes_read) = bufreader.read_until(b'|', &mut buf) {
					if bytes_read != 0 {
						if buf.last() == Some(&b'|') {
							buf.pop();
							let buf_str = String::from_utf8_lossy(&buf);
							if let std::borrow::Cow::Owned(owned_str) = &buf_str {
								println!("MALFORMED UTF-8 in severity: {}", owned_str);
							} else {
								if let Some(rds_log_sev) = RdsLogSeverity::from_str(&buf_str) {
									log_entry.severity = normalize_rds_log_sev(rds_log_sev);
								} else {
									//TODO: Notify of invalid severity?
									println!("MALFORMED Log severity: {}", buf_str);
								}
							}
							buf.clear();
							parser_state = RdsLogParserState::ExpectLogSource;
						} else {
							//TODO: Error, log file ended before delimiter was found.
							break;
						}
					} else {
						//TODO: Log file cut off... Error.
						break;
					}
				} else {
					//TODO: Read error
					break;
				}
			}
			RdsLogParserState::ExpectLogSource => {
				if let Ok(bytes_read) = bufreader.read_until(b'|', &mut buf) {
					if bytes_read != 0 {
						if buf.last() == Some(&b'|') {
							buf.pop();
							let buf_str = String::from_utf8_lossy(&buf);
							if let std::borrow::Cow::Owned(owned_str) = &buf_str {
								println!("MALFORMED UTF-8 in log source: {}", owned_str);
							} else {
								//Successfully read log source; discard it for now
							}
							buf.clear();
							parser_state = RdsLogParserState::ExpectMessage;
						} else {
							//TODO: Error, log file ended before delimiter was found.
							break;
						}
					} else {
						//TODO: Log file cut off... Error.
						break;
					}
				} else {
					//TODO: Read error
					break;
				}
			}
			RdsLogParserState::ExpectMessage => {
				if let Ok(bytes_read) = bufreader.read_until(b'\n', &mut buf) {
					if bytes_read != 0 {
						if buf.last() == Some(&b'\n') {
							parser_state = RdsLogParserState::ExpectDatetimeTentative;
						} else {
							//Log file ended before delimiter was found.
							let message_str = String::from_utf8_lossy(&buf);
							if let std::borrow::Cow::Owned(owned_str) = &message_str {
								println!("MALFORMED UTF-8 in Message: {}", owned_str);
							}
							log_entry.message = message_str.to_string();
							let finalized_log_entry = std::mem::replace(
								&mut log_entry,
								model::LogEntry {
									..Default::default()
								},
							);
							log_entries.push(finalized_log_entry);
							break;
						}
					} else {
						//End of file. Empty message?
						log_entry.message = "".to_string();
						let finalized_log_entry = std::mem::replace(
							&mut log_entry,
							model::LogEntry {
								..Default::default()
							},
						);
						log_entries.push(finalized_log_entry);
						break;
					}
				} else {
					//TODO: Read error
					break;
				}
			}
			RdsLogParserState::ExpectDatetimeTentative => {
				let mut prev_size = buf.len();
				if let Ok(bytes_read) = read_until_pipe_or_newline(&mut bufreader, &mut buf) {
					if bytes_read != 0 {
						if buf.last() == Some(&b'\n') {
							//We are in a multiline message.
							//Just continue reading with ExpectDatetimeTentative.
						} else if buf.last() == Some(&b'|') {
							let mut prev_last_idx = std::cmp::max(prev_size, 1) - 1;
							#[allow(clippy::len_zero)]
							{
								assert!(buf.len() != 0); //We matched Some for the last element
							}
							let last_idx = buf.len() - 1;

							//Skip last byte, as it is the pipe symbol b'|'
							if let Ok(dt_string) =
								std::str::from_utf8(&buf[prev_last_idx..last_idx])
							{
								if let Some(timestamp) = parse_rds_datetime(dt_string) {
									//Trim line ending from the end of the message
									if let Some(b'\n') = buf.get(prev_last_idx) {
										prev_size = prev_last_idx;
										prev_last_idx = std::cmp::max(prev_size, 1) - 1;
									}
									if let Some(b'\r') = buf.get(prev_last_idx) {
										prev_size = prev_last_idx;
									}
									//Emit message
									let message_str = String::from_utf8_lossy(&buf[..prev_size]);
									if let std::borrow::Cow::Owned(owned_str) = &message_str {
										println!("MALFORMED UTF-8 in Message: {}", owned_str);
									}
									log_entry.message = message_str.to_string();
									let finalized_log_entry = std::mem::replace(
										&mut log_entry,
										model::LogEntry {
											..Default::default()
										},
									);
									log_entries.push(finalized_log_entry);

									log_entry.timestamp = timestamp;
									parser_state = RdsLogParserState::ExpectErrcodeOrSeverity;
									buf.clear();
								} else {
									//Cannot parse datetime, so it must be the continuation
									//of a log message...
									//Just continue reading with ExpectDatetimeTentative.
								}
							} else {
								//TODO: Notify of malformed UTF-8?
								println!(
									"MALFORMED UTF-8 in datetime string: {}",
									&String::from_utf8_lossy(&buf)
								);
								parser_state = RdsLogParserState::ExpectErrcodeOrSeverity;
								buf.clear();
							}
						} else {
							//Log file ended before delimiter was found.
							let message_str = String::from_utf8_lossy(&buf);
							if let std::borrow::Cow::Owned(owned_str) = &message_str {
								println!("MALFORMED UTF-8 in Message: {}", owned_str);
							}
							log_entry.message = message_str.to_string();
							let finalized_log_entry = std::mem::replace(
								&mut log_entry,
								model::LogEntry {
									..Default::default()
								},
							);
							log_entries.push(finalized_log_entry);
							break;
						}
					} else {
						//End of log file.
						let message_str = String::from_utf8_lossy(&buf);
						if let std::borrow::Cow::Owned(owned_str) = &message_str {
							println!("MALFORMED UTF-8 in Message: {}", owned_str);
						}
						log_entry.message = message_str.to_string();
						let finalized_log_entry = std::mem::replace(
							&mut log_entry,
							model::LogEntry {
								..Default::default()
							},
						);
						log_entries.push(finalized_log_entry);
						break;
					}
				} else {
					//TODO: Read error
					break;
				}
			}
		}
	}
	root.children = model::LogSourceContents::Entries(log_entries);
	root
}

enum RdsLogParserState {
	ExpectDatetime,
	// Some log files contain an error code.
	// Other log files skip straight to the severity.
	// This is why the parser handles both in this state.
	ExpectErrcodeOrSeverity,
	ExpectSeverity,
	ExpectLogSource,
	ExpectMessage,
	// Some log messages contain line breaks.
	// Therefore, we need to tentatively parse the datetime
	// and if it fails, we are still parsing the message.
	ExpectDatetimeTentative,
}

fn normalize_rds_log_sev(rds_log_sev: RdsLogSeverity) -> model::LogLevel {
	match rds_log_sev {
		RdsLogSeverity::Fatal => model::LogLevel::Critical,
		RdsLogSeverity::Error => model::LogLevel::Error,
		RdsLogSeverity::Warning => model::LogLevel::Warning,
		RdsLogSeverity::Info => model::LogLevel::Info,
		RdsLogSeverity::Debug => model::LogLevel::Debug,
		RdsLogSeverity::Trace => model::LogLevel::Trace,
	}
}

enum RdsLogSeverity {
	Fatal,
	Error,
	Warning,
	Info,
	Debug,
	Trace,
}

impl RdsLogSeverity {
	// Lowercase in files: [Hexagon.Rds.Jobs.log, Hexagon.Rds.Jobs.2.log, Hexagon.Rds.Service.ScanningStatistics.log]
	// Capitalized files: [rds.log, RDSLocalDBDLL.log, Romer.RDS.APINet.log]
	// Uppercase in files: [RDSAgent.log, RDSToolBox.log]
	fn from_str(value: &str) -> Option<RdsLogSeverity> {
		match value {
			"critical" | "Fatal" | "FATAL" => Some(RdsLogSeverity::Fatal), // Note the lowercase "critical", not "fatal" here!
			"error" | "Error" | "ERROR" => Some(RdsLogSeverity::Error),
			"warn" | "Warn" | "WARN" => Some(RdsLogSeverity::Warning),
			"info" | "Info" | "INFO" => Some(RdsLogSeverity::Info),
			"debug" | "Debug" | "DEBUG" => Some(RdsLogSeverity::Debug),
			"trace" | "Trace" | "TRACE" => Some(RdsLogSeverity::Trace),
			_ => None,
		}
	}
}
