//use super::super::model;

extern crate chrono;

/*use chrono::prelude::*;
use std::collections::HashMap;
use std::io::BufReader;
use std::io::Read;
use std::mem;
use std::cmp;


// XLOG parser ----------------------------------------------------------------------

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
*/

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
//<E>   exception details (e.g. stack traces, etc.)
//<A>   application (GUID?)
//<I>   process ID (PID)
//<C>   channel (user level?)
//<S>   session number
//<PIE> private inner exception
//<EN>  error number

//Files are structured as follows: Application_PID_channel_datetime.xlog
//Files with same Application_PID_channel - tuple are concatenated
//in the order they were written into the file.

