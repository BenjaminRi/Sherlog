extern crate chrono;

use chrono::prelude::DateTime;
use chrono::prelude::NaiveDateTime;
use chrono::prelude::Utc;

use std::convert::TryFrom;

// 100-nanosecond offset from 0000-01-01 00:00:00.000 to 1970-01-01 00:00:00.000
const ZERO_OFFSET_100NS: u64 = 621_355_968_000_000_000;
// 1-second offset from 0000-01-01 00:00:00.000 to 1970-01-01 00:00:00.000
const ZERO_OFFSET_1SEC: i64 = 62_135_596_800;

pub fn from_100ns(gcom_datetime: u64) -> Option<chrono::DateTime<Utc>> {
	// `gcom_datetime` is a 100-nanosecond offset from 0000-01-01 00:00:00.000

	const SECONDS_FACTOR: u64 = 10_000_000;
	const NANOSECONDS_FACTOR: u32 = 100;

	if let Some(timestamp_100ns) = gcom_datetime.checked_sub(ZERO_OFFSET_100NS) {
		let ts_sec: u64 = timestamp_100ns / SECONDS_FACTOR; // Regular POSIX timestamp
		let ts_nano: u32 =
			u32::try_from(timestamp_100ns - ts_sec * SECONDS_FACTOR).unwrap() * NANOSECONDS_FACTOR; // Remainder, always fits into u32
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
		// Don't even bother (were there even humans on planet earth back in 1970?)
		// TODO: Handle this case.
		None
	}
}

pub fn to_100ns(datetime: chrono::DateTime<Utc>) -> Option<u64> {
	const SECONDS_FACTOR: u64 = 10_000_000;
	const NANOSECONDS_FACTOR: u32 = 100;

	if let Some(zero_offset_sec) = datetime.timestamp().checked_add(ZERO_OFFSET_1SEC) {
		if let Ok(zero_offset_sec) = u64::try_from(zero_offset_sec) {
			if let Some(gcom_datetime) = zero_offset_sec.checked_mul(SECONDS_FACTOR) {
				// This only fails in fringe cases for large values
				gcom_datetime.checked_add(u64::from(
					datetime.timestamp_subsec_nanos() / NANOSECONDS_FACTOR,
				))
			} else {
				// Year is so large (think thousands of years in the future)
				// that it cannot be represented with a 100ns offset since year 0.
				None
			}
		} else {
			// We are before year 0 (negative value). Unrepresentable in GCOM datetime.
			None
		}
	} else {
		// Year is so incredibly large (approximately 292 billion years)
		// that the addition of the 1970 offset overflows.
		None
	}
}

pub fn add_offset_100ns(
	datetime: chrono::DateTime<Utc>,
	gcom_timespan: i64,
) -> Option<chrono::DateTime<Utc>> {
	//TODO: Use Rust's timespan arithmetic? CAREFUL: Rust has leap seconds, GCOM does not!

	if let Some(gcom_datetime) = to_100ns(datetime) {
		if gcom_timespan == i64::MIN {
			if let Some(gcom_datetime) = gcom_datetime.checked_sub((-(gcom_timespan + 1)) as u64) {
				if let Some(gcom_datetime) = gcom_datetime.checked_sub(1) {
					from_100ns(gcom_datetime)
				} else {
					//Underflow happened while subtracting 1
					None
				}
			} else {
				//Underflow happened while subtracting (i64::MIN + 1)
				None
			}
		} else if gcom_timespan < 0 {
			if let Some(gcom_datetime) = gcom_datetime.checked_sub((-gcom_timespan) as u64) {
				from_100ns(gcom_datetime)
			} else {
				//Underflow happened while subtracting timespan
				None
			}
		} else {
			if let Some(gcom_datetime) = gcom_datetime.checked_add(gcom_timespan as u64) {
				from_100ns(gcom_datetime)
			} else {
				//Overflow happened while adding timespan
				None
			}
		}
	} else {
		//Cannot convert this datetime to GCOM time.
		None
	}
}

pub fn from_timestamp_ms(timestamp_ms: u64) -> Option<chrono::DateTime<Utc>> {
	// `timestamp_ms` is a millisecond offset from 1970-01-01 00:00:00.000
	const SECONDS_FACTOR: u64 = 1000;
	const NANOSECONDS_FACTOR: u32 = 1_000_000;

	let ts_sec: u64 = timestamp_ms / SECONDS_FACTOR;
	let ts_nano: u32 =
		u32::try_from(timestamp_ms - ts_sec * SECONDS_FACTOR).unwrap() * NANOSECONDS_FACTOR; //TODO: reason why this unrwap() is okay
	if let Some(ndt) = NaiveDateTime::from_timestamp_opt(i64::try_from(ts_sec).unwrap(), ts_nano) {
		//TODO: reason why this unrwap() is okay
		Some(DateTime::<Utc>::from_utc(ndt, Utc))
	} else {
		// Malformed or invalid DateTime
		None
	}
}

#[cfg(test)]
mod tests {
	// Test in-file as we cannot test on a binary crate level:
	// https://users.rust-lang.org/t/solved-the-integration-doesnt-run/23078/4
	// https://users.rust-lang.org/t/integration-tests-for-binary-crates/21373
	// https://github.com/rust-lang/cargo/issues/7885
	// https://github.com/rust-lang/book/issues/1940

	use super::*;

	fn validate_gcom_datetime(gcom_datetime: u64, rfc339_nanosec: &str) {
		// Test DateTime correctness
		let date_time = from_100ns(gcom_datetime).expect("Conversion to Rust DateTime failed");
		assert_eq!(
			rfc339_nanosec,
			date_time.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
		);

		// Test round-trip
		let date_time_val = to_100ns(date_time).expect("Conversion back to GCOM DateTime failed");
		assert_eq!(gcom_datetime, date_time_val);
	}

	#[test]
	fn test_conversions() {
		validate_gcom_datetime(637287826990857502, "2020-06-26T15:38:19.085750200Z");
		validate_gcom_datetime(637287826990862498, "2020-06-26T15:38:19.086249800Z");
		validate_gcom_datetime(637287826990867494, "2020-06-26T15:38:19.086749400Z");
		validate_gcom_datetime(637287826990872490, "2020-06-26T15:38:19.087249000Z");
	}

	#[test]
	fn test_offset_addition() {
		let dt1 = from_100ns(637287826990872490).expect("Conversion to Rust DateTime failed"); //2020-06-26T15:38:19.087249000Z

		let dt2 = add_offset_100ns(dt1, -14_988).expect("Addition of offset failed");
		let dt2_val = to_100ns(dt2).expect("Conversion back to GCOM DateTime failed");
		assert_eq!(637287826990857502, dt2_val);
		assert_eq!(
			dt2.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
			"2020-06-26T15:38:19.085750200Z"
		)
	}
}
