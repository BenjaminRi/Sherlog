extern crate chrono;

use chrono::prelude::NaiveDateTime;
use chrono::prelude::DateTime;
use chrono::prelude::Utc;

use std::convert::TryFrom;

pub fn from_100ns(gcom_datetime : u64) -> Option<chrono::DateTime<Utc>> {
	// `gcom_datetime` is a 100-nanosecond offset from 0000-01-01 00:00:00.000
	
	// 100-nanosecond offset from 0000-01-01 00:00:00.000 to 1970-01-01 00:00:00.000
	const TIME_OFFSET: u64 = 621_355_968_000_000_000;
	
	const SECONDS_FACTOR: u64 = 10_000_000;
	const NANOSECONDS_FACTOR: u32 = 100;
	
	if gcom_datetime >= TIME_OFFSET {
		let timestamp_100ns = gcom_datetime - TIME_OFFSET;
		let ts_sec: u64 = timestamp_100ns / SECONDS_FACTOR; //Regular POSIX timestamp
		let ts_nano: u32 = u32::try_from(timestamp_100ns - ts_sec * SECONDS_FACTOR).unwrap() * NANOSECONDS_FACTOR; // Remainder, always fits into u32
		if let Some(ndt) =
			NaiveDateTime::from_timestamp_opt(i64::try_from(ts_sec).unwrap(), ts_nano)
		{
			Some(DateTime::<Utc>::from_utc(ndt, Utc))
		} else {
			// Malformed or invalid DateTime
			None
		}
	} else {
		// Offset before 1970-01-01 00:00:00.000
		None
	}
}

pub fn from_timestamp_ms(timestamp_ms : u64) -> Option<chrono::DateTime<Utc>> {
	// `timestamp_ms` is a millisecond offset from 1970-01-01 00:00:00.000
	const SECONDS_FACTOR: u64 = 1000;
	const NANOSECONDS_FACTOR: u32 = 1_000_000;
	
	let ts_sec: u64 = timestamp_ms / SECONDS_FACTOR;
	let ts_nano: u32 = u32::try_from(timestamp_ms - ts_sec * SECONDS_FACTOR).unwrap() * NANOSECONDS_FACTOR;
	if let Some(ndt) =
		NaiveDateTime::from_timestamp_opt(i64::try_from(ts_sec).unwrap(), ts_nano)
	{
		Some(DateTime::<Utc>::from_utc(ndt, Utc))
	} else {
		// Malformed or invalid DateTime
		None
	}
}
