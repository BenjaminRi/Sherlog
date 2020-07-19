use super::super::model;

use super::datetime_utils;

use std::io::BufRead;
use std::io::BufReader;

// XLOG parser ----------------------------------------------------------------------

pub fn to_log_entries(reader: impl std::io::Read, mut root: model::LogSource) -> model::LogSource {
	let bufreader = BufReader::new(reader);
	let mut log_entries = Vec::<model::LogEntry>::new();
	for line in bufreader.lines() {
		let mut log_entry = model::LogEntry {
			..Default::default()
		};
		//TODO: When is line None? Does that even happen?
		for unit in line.unwrap().split('˫') {
			let offset = unit.find('˩');
			if let Some(offset) = offset {
				let unit_header = &unit[0..offset];
				let unit_value = &unit[offset + 2..]; //Note: +2 because '˩' is 2 Byte in UTF-8

				match unit_header {
					"<T>" => {
						if let Ok(gcom_datetime) = unit_value.parse::<u64>() {
							if let Some(datetime) = datetime_utils::from_100ns(gcom_datetime) {
								log_entry.timestamp = datetime;
							} else {
								//TODO: Notify of invalid datetime?
								println!("MALFORMED Log 100ns datetime: {}", gcom_datetime);
							}
						} else {
							//TODO: Notify of invalid time?
							println!("MALFORMED Log 100ns value: {}", unit_value);
						}
					}
					"<L>" => {
						if let Some(xlog_sev) = XlogSeverity::from_str(unit_value) {
							log_entry.severity = normalize_xlog_sev(xlog_sev);
						} else {
							//TODO: Notify of invalid severity?
							println!("INVALID Log severity: {}", unit_value);
						}
					}
					"<M>" => {
						log_entry.message = unit_value.to_string();
					}
					_ => {
						//TODO: Notify of invalid kind?
						//println!("UNRECOGNIZED kind: {}", &unit_header);
					}
				}

			//println!("Header: [{}] Value: [{}]", unit_header, unit_value);
			} else {
				println!("ERROR: NO DELMIMTER FOUND IN {}", unit);
			}
		}

		log_entries.push(log_entry);
	}

	root.children = model::LogSourceContents::Entries(log_entries);
	root
}

#[rustfmt::skip]
fn normalize_xlog_sev(xlog_sev: XlogSeverity) -> model::LogLevel {
	match xlog_sev {
		XlogSeverity::AppStart  => model::LogLevel::Info, //loss of information!
		XlogSeverity::AppStop   => model::LogLevel::Info, //loss of information!
		XlogSeverity::Info      => model::LogLevel::Info,
		XlogSeverity::Warning   => model::LogLevel::Warning,
		XlogSeverity::Error     => model::LogLevel::Error,
		XlogSeverity::Exception => model::LogLevel::Error, //loss of information!
		XlogSeverity::Debug     => model::LogLevel::Debug,
	}
}

enum XlogSeverity {
	AppStart,
	AppStop,
	Info,
	Warning,
	Error,
	Exception,
	Debug,
}

impl XlogSeverity {
	#[rustfmt::skip]
	fn from_str(value: &str) -> Option<XlogSeverity> {
		match value {
			"AppStart"  => Some(XlogSeverity::AppStart),
			"AppStop"   => Some(XlogSeverity::AppStop),
			"Info"      => Some(XlogSeverity::Info),
			"Warning"   => Some(XlogSeverity::Warning),
			"Error"     => Some(XlogSeverity::Error),
			"Exception" => Some(XlogSeverity::Exception),
			"Debug"     => Some(XlogSeverity::Debug),
			_ => None,
		}
	}
}

//Note: This absolutely lunatic format uses various weird Unicode
//symbols to encode delimiters and newlines. Apparently its author
//has never heard of escaping, size-prefixing or ASCII separator
//symbols. Be as it may, we need to parse this format and transform
//it into a usable data structure.

//Example xlog log line:
//<T>˩637055156092730381˫<L>˩Info˫<M>˩LoggerService: Started.˫<A>˩ApplicationX˫<I>˩14016˫<C>˩System

//Note that messages containing '˫' and '˩' cause severe parser
//issues. The original parser splits each row at '˫', and then
//for each unit it splits at '˩' and assumes that the first hit is
//the unit header and the second hit is the unit value.

//Record separator: '\n'
//Unit separator: '˫'
//Unit header separator: '˩'
//Newline encoding: Newlines,'\r','\n' to '˪'

//<T>   timestamp
//<L>   log level (AppStart, AppStop, Info, Warning, Error, Exception, Debug)
//<M>   message
//<E>   exception details (e.g. stack traces, etc.) ('˪' is newline!)
//<A>   application (GUID?)
//<I>   process ID (PID)
//<C>   channel (user level?)
//<S>   session number
//<PIE> private inner exception
//<EN>  error number

//Files are structured as follows: Application_PID_channel_datetime.xlog
//1. Files with same Application_PID_channel - concatenate /group in temporal order.
//2. Group the collections formed in (1) by channel
//3. Parse & merge sort together
