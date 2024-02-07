extern crate chrono;
extern crate zip;

use chrono::prelude::DateTime;
use chrono::prelude::NaiveDateTime;
use chrono::prelude::Utc;
use std::path::PathBuf;

use super::super::model;
use super::datetime_utils;
use super::glog;
use super::rds_log;
use super::scanlib_log;
use super::xlog;

use std::collections::HashMap;
use std::mem;

static SFILE_PASSWORD: Option<&'static str> = option_env!("SFILE_PASSWORD");

pub fn from_file(path: &std::path::PathBuf) -> Result<model::LogSource, std::io::Error> {
	let file = std::fs::File::open(&path)?;
	let mut archive = zip::ZipArchive::new(file)?;

	let mut glog_files = Vec::new();

	let mut client_child_sources = Vec::new();
	let mut rds_child_sources = Vec::new();
	let mut scanlib_child_sources = Vec::new();
	let mut child_sources = Vec::new();
	child_sources.reserve(archive.len());

	for i in 0..archive.len() {
		let file = if let Some(password) = SFILE_PASSWORD {
			archive.by_index_decrypt(i, password.as_bytes())?.unwrap() //TODO: 21.06.2020: Handle InvalidPassword!
		} else {
			archive.by_index(i)?
		};
		let outpath = PathBuf::from(file.name());
		let stem = outpath.file_stem().unwrap();
		let stem = stem.to_string_lossy();
		//log::info!("File contained: {}", &stem);

		// .ZIP specification, Version: 6.3.9, Paragraph 4.4.17 file name: (Variable)
		// All slashes MUST be forward slashes '/' as opposed to backwards slashes '\' [...]
		//
		// Therefore, we can safely match folders with `/`
		if outpath.starts_with("RDS/") {
			if let Some(extension) = outpath.extension() {
				match extension.to_string_lossy().as_ref() {
					"log" => {
						if stem.starts_with("ScanLib_") {
							log::info!("Log file (ScanLib): {}", &stem);
							let root = model::LogSource {
								name: stem.to_string(),
								children: {
									model::LogSourceContents::Entries(Vec::<model::LogEntry>::new())
								},
							};
							scanlib_child_sources.push(scanlib_log::to_log_entries(file, root));
						} else {
							log::info!("Log file (RDS): {}", &stem);
							let root = model::LogSource {
								name: stem.to_string(),
								children: {
									model::LogSourceContents::Entries(Vec::<model::LogEntry>::new())
								},
							};
							rds_child_sources.push(rds_log::to_log_entries(file, root));
						}
					}
					unknown_extension => {
						log::warn!("Unknown extension in RDS folder: {}", unknown_extension)
					}
				}
			}
		} else if let Some(extension) = outpath.extension() {
			match extension.to_string_lossy().as_ref() {
				"glog" => {
					glog_files.push(ZipEntry {
						name: stem.to_string(),
						group_name: get_group_name(&stem),
						index: i,
					});
				}
				"xlog" => {
					//log::info!("XLOG: {}", &stem);
					let root = model::LogSource {
						name: stem.to_string(),
						children: {
							model::LogSourceContents::Entries(Vec::<model::LogEntry>::new())
						},
					};
					client_child_sources.push(xlog::to_log_entries(file, root));
				}
				_ => (),
			}
		}
	}

	//Arrange Client logs into their respective channels
	let mut client_log_sources = HashMap::<String, model::LogSource>::new();
	for file_source in client_child_sources {
		let mut rsplitn_name = file_source.name.rsplitn(4, '_');

		let _date_time = if let Some(date_time) = rsplitn_name.next() {
			// Parse date_time. Example: "2021-03-09-08-07-25-8527"
			let padded_date_time = format!("{}00000", date_time); // Pad fractional seconds to nanoseconds
			if let Ok(ndt) =
				NaiveDateTime::parse_from_str(&padded_date_time, "%Y-%m-%d-%H-%M-%S-%f")
			{
				DateTime::<Utc>::from_utc(ndt, Utc)
			} else {
				// Invalid date_time format, parse error
				DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp_opt(0, 0).unwrap(), Utc)
			}
		} else {
			// rsplitn always returns at least one iterator element
			unreachable!()
		};

		let channel_name = if let Some(channel_name) = rsplitn_name.next() {
			channel_name
		} else {
			// Too few iterator elements (underscores)
			// Use entire file name (without ".xlog" extension) as fallback
			&file_source.name
		};

		let _process_id = if let Some(process_id) = rsplitn_name
			.next()
			.map_or(None, |pid_str| pid_str.parse::<u32>().ok())
		{
			process_id // Windows PIDs are stored in nonzero DWORD (u32)
		} else {
			0 // Invalid PID
		};

		let _application_name = if let Some(application_name) = rsplitn_name.next() {
			application_name
		} else {
			"Unknown application name"
		};

		//log::info!("{:?} {:?} {:?} {:?}", _date_time, channel_name, _process_id, _application_name);

		let source_option = client_log_sources.get_mut(channel_name);
		if let Some(source) = source_option {
			//Log sub-source exists, push contents
			let children = &mut source.children;
			match children {
				model::LogSourceContents::Entries(v) => {
					if let model::LogSourceContents::Entries(mut entries) = file_source.children {
						v.append(&mut entries);
					} else {
						unreachable!(); //If this panics, there is a bug in the XLOG parser
					}
				}
				_ => unreachable!(), //We only insert LogSourceContents::Entries
			}
		} else {
			//Log sub-source does not yet exist
			client_log_sources.insert(
				channel_name.to_string(),
				model::LogSource {
					name: channel_name.to_string(),
					children: {
						if let model::LogSourceContents::Entries(entries) = file_source.children {
							model::LogSourceContents::Entries(entries)
						} else {
							unreachable!(); //If this panics, there is a bug in the XLOG parser
						}
					},
				},
			);
		}
	}
	let mut client_child_sources = Vec::new();
	for (_, sub_source) in client_log_sources {
		client_child_sources.push(sub_source);
	}

	//Sort glog files by group (a group is the file name with its ring buffer ID / overview suffix removed)
	//Inside a group, reverse sort by file name (and thus buffer ID) to get chronological ordering of files.
	//The reason for this is that higher ring buffer ID means the file is older. Older log entries come first.
	//Note: Overview logs are interspersed with normal logs, therefore no chronological order between
	//the log entries of normal logs and overview logs can be established without inspecting timestamps.
	//Note: Sensor has logSync and logAsync, leading to minor violations of chronological order inside a file
	//and even e.g. between the end of one file and the start of another.
	//Due to these facts, this ordering is only mostly chronological with regards to individual log entries.
	//This chronological ordering is required to perform timestamp corrections for devices that do not have
	//a real-time clock and thus start with 1970 timestamps on boot-up before distributed clock time is set.
	//It is also required to preserve the order of log entries that were logged with the exact same timestamp.
	//
	//Example ordering:
	//Group name           File name
	//--------------------------------------
	//adm_LoggerAdm_p      adm_LoggerAdm_2_p
	//adm_LoggerAdm_p      adm_LoggerAdm_1_p
	//adm_LoggerAdm_v      adm_LoggerAdm_1_v
	//contr_FtcManager     contr_FtcManager_ov
	//contr_FtcManager     contr_FtcManager
	//contr_Hwa            contr_Hwa_4
	//contr_Hwa            contr_Hwa_3
	//contr_Hwa            contr_Hwa_2
	//contr_Hwa            contr_Hwa_1
	//contr_Hwa            contr_Hwa
	//
	//Note the sorting subtleties, especially with _p and _v logs, where the ID is not a suffix.

	glog_files.sort_unstable_by(|a, b| {
		a.group_name
			.cmp(&b.group_name)
			.then(a.name.cmp(&b.name).reverse())
	});

	let mut deque = std::collections::VecDeque::new();
	let mut last_group = "".to_string();
	for file in glog_files {
		if last_group != file.group_name {
			if !deque.is_empty() {
				let deque = mem::replace(&mut deque, std::collections::VecDeque::new());
				let reader = ConcatZipReader::new(&mut archive, deque);
				let root = model::LogSource {
					name: last_group,
					children: { model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) },
				};
				child_sources.push(glog::to_log_entries(reader, root));
			}
			log::info!("--------------------");
			log::info!("Glog file: {:?}", file);
			last_group = file.group_name;
		} else {
			log::info!("Glog file: {:?}", file);
		}
		deque.push_back(file.index);
	}
	if !deque.is_empty() {
		let deque = mem::replace(&mut deque, std::collections::VecDeque::new());
		let reader = ConcatZipReader::new(&mut archive, deque);
		let root = model::LogSource {
			name: last_group,
			children: { model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) },
		};
		child_sources.push(glog::to_log_entries(reader, root));
	}

	let mut contr_child_sources = Vec::new();
	let mut sensor_child_sources: std::vec::Vec<model::LogSource> = Vec::new();
	let mut cbox_child_sources = Vec::new();
	let mut probe_child_sources = Vec::new();
	let mut unknown_child_sources = Vec::new();

	for mut source in child_sources {
		//Controller logs
		if source.name.starts_with("contr_") {
			//Remove "contr_" to make it look nicer
			source.name = source.name.split_off(6);
			contr_child_sources.push(source);
			continue;
		}

		//Sensor logs
		let mut iter = source.name.splitn(2, '_');
		let board_name = iter.next().unwrap(); //First element always exists
		if let Some(log_name) = iter.next() {
			if board_name == "axis" ||
				board_name == "sensorbase" ||
				board_name == "telescope" ||
				board_name == "trigger" ||
				board_name == "adm" || // G
				board_name == "laseroven" || // G
				board_name == "wfd"	|| // W
				board_name == "dynamicadm" || // P
				board_name == "icbpower" || // P
				board_name.starts_with("cfm") || // L
				board_name == "laserctl" || // L
				board_name == "wlanmodule" || // L
				false
			{
				if sensor_child_sources.is_empty()
					|| sensor_child_sources.last().unwrap().name != board_name
				{
					let board_name_string = board_name.to_string();
					source.name = log_name.to_string();
					sensor_child_sources.push(model::LogSource {
						name: board_name_string,
						children: { model::LogSourceContents::Sources(vec![source]) },
					});
				} else {
					source.name = log_name.to_string();
					if let model::LogSourceContents::Sources(sources) =
						&mut sensor_child_sources.last_mut().unwrap().children
					{
						sources.push(source)
					} else {
						//We only pushed model::LogSourceContents::Sources
						unreachable!();
					}
				}
				continue;
			}
		}

		//Connect Box logs
		if source.name.starts_with("connectbox_") {
			//Remove "connectbox_" to make it look nicer
			source.name = source.name.split_off(11);
			cbox_child_sources.push(source);
			continue;
		}

		//Probe logs
		if source.name.starts_with("ap21_") {
			//Remove "ap21_" to make it look nicer
			source.name = source.name.split_off(5);
			probe_child_sources.push(source);
			continue;
		}

		//Unknown logs
		unknown_child_sources.push(source);
	}

	//Case insensitive sort by log source name
	contr_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	sensor_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	client_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	cbox_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	probe_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	rds_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	scanlib_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	unknown_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

	for mut source in &mut sensor_child_sources {
		adjust_sensor_timestamps(&mut source);
	}
	for mut source in &mut cbox_child_sources {
		adjust_sensor_timestamps(&mut source);
	}
	for mut source in &mut probe_child_sources {
		adjust_sensor_timestamps(&mut source);
	}

	let contr_logs = model::LogSource {
		name: "Controller".to_string(),
		children: { model::LogSourceContents::Sources(contr_child_sources) },
	};
	let sensor_logs = model::LogSource {
		name: "Sensor".to_string(),
		children: { model::LogSourceContents::Sources(sensor_child_sources) },
	};
	let client_logs = model::LogSource {
		name: "Client".to_string(),
		children: { model::LogSourceContents::Sources(client_child_sources) },
	};

	let mut sources_vec = vec![client_logs, contr_logs, sensor_logs];

	if !cbox_child_sources.is_empty() {
		let cbox_logs = model::LogSource {
			name: "Connect Box".to_string(),
			children: { model::LogSourceContents::Sources(cbox_child_sources) },
		};
		sources_vec.push(cbox_logs);
	}

	if !probe_child_sources.is_empty() {
		let probe_logs = model::LogSource {
			name: "Probe".to_string(),
			children: { model::LogSourceContents::Sources(probe_child_sources) },
		};
		sources_vec.push(probe_logs);
	}

	if !rds_child_sources.is_empty() {
		let rds_logs = model::LogSource {
			name: "RDS".to_string(),
			children: { model::LogSourceContents::Sources(rds_child_sources) },
		};
		sources_vec.push(rds_logs);
	}

	if !scanlib_child_sources.is_empty() {
		let scanlib_logs = model::LogSource {
			name: "ScanLib".to_string(),
			children: { model::LogSourceContents::Sources(scanlib_child_sources) },
		};
		sources_vec.push(scanlib_logs);
	}

	if !unknown_child_sources.is_empty() {
		let unknown_logs = model::LogSource {
			name: "Unknown".to_string(),
			children: { model::LogSourceContents::Sources(unknown_child_sources) },
		};
		sources_vec.push(unknown_logs);
	}

	Ok(model::LogSource {
		name: path.file_name().unwrap().to_string_lossy().to_string(),
		children: { model::LogSourceContents::Sources(sources_vec) },
	})
}

fn adjust_sensor_timestamps(source: &mut model::LogSource) {
	match &mut source.children {
		model::LogSourceContents::Sources(v) => {
			for mut source in v {
				adjust_sensor_timestamps(&mut source);
			}
		}
		model::LogSourceContents::Entries(v) => {
			log::info!("Adjust sensor timestamps: {:?}", source.name);

			#[derive(Debug, PartialEq)]
			struct Correction {
				session_id: u32,
				delta: i64,
			}
			let mut active_correction: Option<Correction> = None;
			// Reverse iterate, from the newest to the oldest entry
			for entry in v.iter_mut().rev() {
				if let Some(field) = entry.custom_fields.get("SessionId") {
					if let model::CustomField::UInt32(session_id) = field {
						// Some log entries say:
						// Setting EtherCAT time [delta = 1562060032100954112 ns].
						// Others say (omitting the dot):
						// Setting EtherCAT time [delta = 1562060032100954112 ns]
						// We need to handle both...
						if entry.message.starts_with("Setting EtherCAT time [delta = ")
							&& (entry.message.ends_with(" ns].") || entry.message.ends_with(" ns]"))
						{
							let delta = entry.message.split(' ').nth(5).unwrap(); //we can unwrap here because we verified the format above
							if let Ok(delta) = delta.parse::<i64>() {
								let old_correction = mem::replace(
									&mut active_correction,
									Some(Correction {
										session_id: *session_id,
										delta,
									}),
								);

								let old_session_id_opt =
									if let Some(old_correction) = &old_correction {
										Some(old_correction.session_id)
									} else {
										None
									};

								if old_correction == active_correction {
									log::warn!(
										"Overwriting EtherCAT Time with same content! {:?}",
										active_correction
									);
								} else if old_session_id_opt == Some(*session_id) {
									log::warn!(
										"Overwriting EtherCAT Time! Old: {:?}, New: {:?}",
										old_correction,
										active_correction
									);
								} else {
									// This is the happy path for reading timestamp corrections.
									// Happens when:
									// - The very first correction is read
									// - A valid correction is read after the last one was invalidated by e.g. a session change
									// - A valid correction replaces a previous valid correction due to session change
									//log::info!(
									//	"Read fresh timestamp correction: {:?}",
									//	active_correction
									//);
								}
							} else {
								log::warn!("could not parse EtherCAT timestamp {}", delta);
								active_correction = None;
							}
						} else {
							if let Some(correction) = &active_correction {
								if *session_id == correction.session_id {
									// Timestamps before 01-01-2001 00:00:00.000000 are not realistic because the device did not exist back then.
									// We can safely assume that these are relative timestamps that are not yet corrected with EtherCAT time.
									// It is also reasonable to assume that a device receives its EtherCAT time within 2 years (or never).
									if entry.timestamp
										< DateTime::<Utc>::from_utc(
											NaiveDateTime::from_timestamp_opt(978_300_000, 0)
												.unwrap(),
											Utc,
										) {
										//Divide delta by 100 to convert from 1ns to 100ns ticks, which is the default GCOM timespan measurement
										if let Some(corrected_timestamp) =
											datetime_utils::add_offset_100ns(
												entry.timestamp,
												correction.delta / 100,
											) {
											entry.timestamp = corrected_timestamp;
										} else {
											log::warn!(
												"could not correct timestamp with offset: {}",
												correction.delta
											);
										}
									}
								} else {
									// We moved on to a different session. Scrap active timestamp correction.
									active_correction = None;
								}
							} else {
								// This either happens if we encounter an already corrected timestamp and haven't yet
								// encountered the log entry that specifies the time delta.
								// Or else, it happens if the bus never connected, so the device never got the EtherCAT offset.
								// This second case is also a normal thing to occur over the lifetime of a device,
								// but we have to think about how to sort these log lines as their timestamp remains around 1970.
								//log::warn!("Could not find EtherCAT offset for {}!", entry.message);
							}
						}
					} else {
						panic!("Wrong type for session ID!");
					}
				} else {
					// "sensorbase_BaseboardSpecialLogs_1_v.glog" lacks session ID, these logs are special
					log::warn!(
						"No session ID found for sensor log entry: {}",
						entry.message
					);
					active_correction = None;
				}
			}
		}
	}
}

struct ConcatZipReader<'a, R: std::io::Read + std::io::Seek> {
	archive: &'a mut zip::ZipArchive<R>,
	file: Option<zip::read::ZipFile<'a>>,
	indices: std::collections::VecDeque<usize>,
}

impl<'a, R: std::io::Read + std::io::Seek> ConcatZipReader<'a, R> {
	fn new(
		archive: &'a mut zip::ZipArchive<R>,
		indices: std::collections::VecDeque<usize>,
	) -> ConcatZipReader<'a, R> {
		ConcatZipReader {
			archive,
			file: None,
			indices,
		}
	}
}

impl<'a, R: std::io::Read + std::io::Seek> std::io::Read for ConcatZipReader<'a, R> {
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
		loop {
			if let Some(f) = &mut self.file {
				let result = f.read(buf);
				if let Ok(bytes) = result {
					if bytes == 0 {
						//file exhausted, retry with next file
						self.file = None;
						continue;
					}
				}
				//ordinary read from the file
				return result;
			} else {
				match self.indices.pop_front() {
					Some(idx) => {
						//Need to open new file
						let f = if let Some(password) = SFILE_PASSWORD {
							self.archive
								.by_index_decrypt(idx, password.as_bytes())?
								.unwrap() //TODO: 21.06.2020: Handle InvalidPassword!
						} else {
							self.archive.by_index(idx)?
						};
						unsafe {
							//Due to the fact that file references archive and both are in the same struct,
							//this cannot be done in safe Rust
							self.file = Some(std::mem::transmute::<
								zip::read::ZipFile<'_>,
								zip::read::ZipFile<'a>,
							>(f));
						}
						//retry with newly opened file
						continue;
					}
					None => {
						//file list exhausted, end of reader
						return Ok(0);
					}
				}
			}
		}
	}
}

#[derive(Debug)]
struct ZipEntry {
	name: String,
	group_name: String,
	index: usize,
}

fn get_group_name(s: &str) -> String {
	let mut s = s.to_string();
	let storage_type = if s.ends_with("_v") {
		//Virtual (in RAM)
		"_v"
	} else if s.ends_with("_p") {
		//Persistent (on Flash storage)
		"_p"
	} else {
		//Unknown storage type
		""
	};

	s.truncate(s.len() - storage_type.len());

	if let Some(offset) = s.rfind('_') {
		if s[offset + 1..s.len()].parse::<u32>().is_ok() {
			//Numbered logfile. Discard ring buffer index.
			s.truncate(offset);
		}
	}

	if s.ends_with("_ov") {
		//Overview logs are merged with normal logs.
		s.truncate(s.len() - "_ov".len());
	}

	//Restore storage type suffix
	s.push_str(storage_type);
	s
}
