extern crate zip;

use super::super::model;
use super::glog;
use super::xlog;

use std::collections::HashMap;

static SFILE_PASSWORD: Option<&'static str> = option_env!("SFILE_PASSWORD");

pub fn from_file(path: &std::path::PathBuf) -> Result<model::LogSource, std::io::Error> {
	let file = std::fs::File::open(&path)?;
	let mut archive = zip::ZipArchive::new(file)?;

	let mut client_child_sources = Vec::new();
	let mut contr_child_sources = Vec::new();
	let mut sensor_child_sources = Vec::new();
	let mut unknown_child_sources = Vec::new();
	
	for i in 0..archive.len() {
		let file = if let Some(password) = SFILE_PASSWORD {
			archive.by_index_decrypt(i, password.as_bytes())?
		} else {
			archive.by_index(i)?
		};
		let outpath = file.sanitized_name();
		let stem = outpath.file_stem().unwrap();
		let stem = stem.to_string_lossy();

		//println!("File contained: {}", &stem);
		if let Some(extension) = outpath.extension() {
			match extension.to_string_lossy().as_ref() {
				"glog" => {
					println!("GLOG: {}", &stem);
					let root = model::LogSource {
						name: stem.to_string(),
						children: { model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) },
					};
					
					let log_entry = glog::to_log_entries(file, root);
					
					let board_name = if let Some(offset) = stem.find('_') {
						&stem[0..offset]
					} else {
						""
					};
					
					match board_name {
						"contr" => contr_child_sources.push(log_entry),
						"adm" | //G
						"axis" |
						"laseroven" | //G
						"sensorbase" |
						"telescope" |
						"trigger" |
						"wfd" //W 
							=> sensor_child_sources.push(log_entry),
						_ => unknown_child_sources.push(log_entry),
					}
				}
				"xlog" => {
					//println!("XLOG: {}", &stem);
					let root = model::LogSource {
						name: stem.to_string(),
						children: { model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) },
					};
					client_child_sources.push(xlog::to_log_entries(file, root));
				}
				_ => (),
			}
		}
	}
	
	println!("--- Parsing done. Arranging log sources and entries");
	
	//Arrange Client logs into their respective channels
	let mut client_log_sources = HashMap::<String, model::LogSource>::new();
	for file_source in client_child_sources {
		let split_name = file_source.name.split("_");
		let channel_name = if let Some(channel) = split_name.skip(2).take(1).next() {
			channel
		} else {
			"Unknown"
		};
		
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
	
	
	struct LogSourceMap {
		name: String,
		children: HashMap<String, model::LogSource>,
	}


	contr_child_sources.sort_by(|a, b| a.name.cmp(&b.name));
	let mut contr_tmp_sources: std::vec::Vec<LogSourceMap> = Vec::new();
	contr_tmp_sources.reserve(contr_child_sources.len());
	for mut source in contr_child_sources.into_iter() {
		//Controller logs
		if source.name.starts_with("contr_") {
			//Remove "contr_" to make it look nicer
			source.name = source.name.split_off(6);
		}
		strip_contr_suffix(&mut source.name);
		
		if contr_tmp_sources.last().is_some() && contr_tmp_sources.last().unwrap().name == source.name {
			let last_source_map = contr_tmp_sources.last_mut().unwrap();
			//we have last_source_map, which has HashMap<String, model::LogSource>, and we need to match log sources
			let children_vec = match source.children {
				model::LogSourceContents::Entries(_) => unreachable!(),
				model::LogSourceContents::Sources(v) => v,
			};
			for child in children_vec {
				if let Some(dest_source) = last_source_map.children.get_mut(&child.name) {
					match &mut dest_source.children {
						model::LogSourceContents::Entries(v_to) => {
							match child.children {
								model::LogSourceContents::Entries(mut v_from) => {
									v_to.append(&mut v_from);
								},
								model::LogSourceContents::Sources(_) => unreachable!(),
							}
						},
						model::LogSourceContents::Sources(_) => unreachable!(),
					};
				} else {
					last_source_map.children.insert(child.name.clone(), child);
				}
			}
		} else {
			match source.children {
				model::LogSourceContents::Entries(_) => println!("ERROR: File with entries!: {}", source.name), //TODO: Handle file with just entries...?
				model::LogSourceContents::Sources(children_vec) => {
					let mut children_map = HashMap::new();
					for child in children_vec {
						children_map.insert(child.name.clone(), child);
					}
					let source_map = LogSourceMap {
						name: source.name,
						children: children_map,
					};
					contr_tmp_sources.push(source_map);
				},
			}
		}
	}
	
	contr_child_sources = Vec::new();
	contr_child_sources.reserve(contr_tmp_sources.len());
	for source in contr_tmp_sources {
		let mut new_children = Vec::new();
		new_children.reserve(source.children.len());
		for (_, sub_source) in source.children {
			new_children.push(sub_source);
		}
		new_children.sort_by(|a, b| a.name.cmp(&b.name));
		let new_source = model::LogSource {
			name: source.name,
			children: model::LogSourceContents::Sources(new_children),
		};
		contr_child_sources.push(new_source);
	}
	
	//contr_child_sources = contr_tmp_sources;

	/*for mut source in sensor_child_sources {
		//Sensor logs
		let mut iter = source.name.splitn(2, '_');
		let board_name = iter.next().unwrap(); //First element always exists
		if let Some(log_name) = iter.next() {
			if board_name == "adm" || //G
				board_name == "axis" ||
				board_name == "laseroven" || //G
				board_name == "sensorbase" ||
				board_name == "telescope" ||
				board_name == "trigger" ||
				board_name == "wfd"
			//W
			{
				if sensor_child_sources.is_empty()
					|| !(sensor_child_sources.last().unwrap().name == board_name)
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
	}*/
	
	//Case insensitive sort by log source name
	client_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	contr_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	sensor_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	unknown_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	
	let mut sources_vec = Vec::new();
	sources_vec.reserve(4);
	
	//Add sources in alphabetical order:
	if !client_child_sources.is_empty() {
		let client_logs = model::LogSource {
			name: "Client".to_string(),
			children: { model::LogSourceContents::Sources(client_child_sources) },
		};
		sources_vec.push(client_logs);
	}

	if !contr_child_sources.is_empty() {
		let contr_logs = model::LogSource {
			name: "Controller".to_string(),
			children: { model::LogSourceContents::Sources(contr_child_sources) },
		};
		sources_vec.push(contr_logs);
	}
	
	if !sensor_child_sources.is_empty() {
		let sensor_logs = model::LogSource {
			name: "Sensor".to_string(),
			children: { model::LogSourceContents::Sources(sensor_child_sources) },
		};
		sources_vec.push(sensor_logs);
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

enum LogStorageType {
	Persistent, //on flash or disk storage
	Volatile, //in RAM
}

struct SensorGlogType {
	buffer_idx: Option<u32>,
	persistence: Option<LogStorageType>,
}

struct ContrGlogType {
	overview: bool,
}

fn strip_contr_suffix(s: &mut String) -> ContrGlogType {
	if let Some(offset) = s.rfind('_') {
		if let Ok(_) = &s[offset + 1..s.len()].parse::<u32>() {
			//Numbered logfile with ring buffer index.
			//The first logfile in the ring buffer has no index.
			//Note: For contr_ files, this might also be
			//the process log id! Due to legacy, this can
			//never be distinguished reliably any more.
			//Example:
			//contr_telescope.glog
			//contr_telescope_1.glog
			//contr_telescope_2.glog
			//contr_Telescope_10061.glog
			s.truncate(offset);
		}
	}

	if s.ends_with("_ov") {
		//Overview logs are merged with normal logs.
		//This is a type of log that rolls over slower
		//and preserves important messages longer.
		s.truncate(s.len() - "_ov".len());
		ContrGlogType { overview: true }
	} else {
		ContrGlogType {overview: false }
	}
}

fn strip_sensor_suffix(s: &mut String) -> SensorGlogType {
	let (storage_type, persistence) = if s.ends_with("_v") {
		("_v", Some(LogStorageType::Volatile))
	} else if s.ends_with("_p") {
		("_p", Some(LogStorageType::Persistent))
	} else {
		//Unknown storage type
		//This should never happen, but be robust anyway
		("", None)
	};

	s.truncate(s.len() - storage_type.len());

	let buffer_idx = if let Some(offset) = s.rfind('_') {
		if let Ok(buffer_idx) = &s[(offset + 1)..s.len()].parse::<u32>() {
			s.truncate(offset);
			Some(*buffer_idx)
		} else {
			//Ringbuffer index cannot be parsed
			//This should never happen, but be robust anyway
			None
		}
	} else {
		//No ringbuffer index exists
		//This should never happen, but be robust anyway
		None
	};

	//Restore storage type suffix
	s.push_str(storage_type);
	
	SensorGlogType {
		buffer_idx,
		persistence,
	}
}
