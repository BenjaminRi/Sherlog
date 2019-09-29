extern crate gio;
extern crate gtk;
extern crate log;
extern crate chrono;

use gio::prelude::*;
use gtk::prelude::*;

#[allow(unused_imports)]
use gtk::{
    ApplicationWindow, ButtonsType, CellRendererPixbuf, CellRendererText, DialogFlags,
    MessageDialog, MessageType, Orientation, TreeStore, TreeView, TreeViewColumn, WindowPosition,
};

use std::env::args;

use std::fs::File;
use std::io::{BufRead, BufReader};
use chrono::prelude::*;

struct LogEntry {
	timestamp : chrono::DateTime<Utc>,
	severity : log::Level,
	message : String,
}

enum LogSourceContents {
	Sources(Vec::<LogSource>),
	Entries(Vec::<LogEntry>),
}

struct LogSource {
	name : String,
	children : LogSourceContents,
}

impl Default for LogEntry {
    fn default() -> LogEntry {
        LogEntry {
            timestamp: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
            severity : log::Level::Error,
            message: "".to_string(),
        }
    }
}

// GLOG parser ----------------------------------------------------------------------

fn line_to_log_entry(line: &str) -> LogEntry {
	let mut log_entry = LogEntry { ..Default::default() };
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
						"tq" => GlogSectionKind::TimestampUs,
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
							GlogSectionKind::TimestampUs => {
							
							},
							GlogSectionKind::Severity => {
							
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

#[derive(Debug, PartialEq)]
enum GlogSectionKind {
	TimestampUs,
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

fn append_text_column(tree: &TreeView, column_id: i32) {
    let column = TreeViewColumn::new();
    let cell = CellRendererText::new();

    // https://lazka.github.io/pgi-docs/Gtk-3.0/classes/TreeViewColumn.html#Gtk.TreeViewColumn.set_sort_indicator
    column.pack_start(&cell, true);
    column.add_attribute(&cell, "text", column_id);
	//column.set_title("Testtitle");
	//column.set_sort_indicator(true);
	//column.set_clickable(true);
    tree.append_column(&column);
}

enum Columns {
    Active = 0,
	Text = 1,
}

fn fixed_toggled<W: IsA<gtk::CellRendererToggle>>(
    model: &gtk::TreeStore,
    _w: &W,
    path: gtk::TreePath,
) {
    let iter = model.get_iter(&path).unwrap();
    let mut fixed = model
        .get_value(&iter, Columns::Active as i32)
        .get::<bool>()
        .unwrap();
    fixed = !fixed;
    model.set_value(&iter, Columns::Active as u32, &fixed.to_value());
}

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);
	//window.set_icon_from_file("../images/sherlog_icon.png");
	
	let file = File::open("../logfiles/example.glog").expect("Could not open file");
    let mut reader = BufReader::new(file);

	let mut log_source = LogSource {name: "Root LogSource".to_string(), children: {LogSourceContents::Sources(Vec::<LogSource>::new()) } };
	let mut log_entries = Vec::new();
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

    window.set_title("Sherlog");
    window.set_border_width(10);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(600, 400);
	
	// left pane
    let left_tree = TreeView::new();
    let left_store = TreeStore::new(&[gtk::Type::Bool, String::static_type()]);

    left_tree.set_model(Some(&left_store));
    left_tree.set_headers_visible(true);
	
	// Column for fixed toggles
    {
        let renderer = gtk::CellRendererToggle::new();
		let model_clone = left_store.clone();
		renderer.connect_toggled(move |w, path| fixed_toggled(&model_clone, w, path));
        let column = gtk::TreeViewColumn::new();
        column.pack_start(&renderer, true);
        column.set_title("Fixed?");
        column.add_attribute(&renderer, "active", Columns::Active as i32);
        column.set_sizing(gtk::TreeViewColumnSizing::Fixed);
		column.set_title("Testtitle");
		column.set_sort_indicator(true);
		column.set_clickable(true);
        //column.set_fixed_width(50);
		
		let cell = CellRendererText::new();
		column.pack_start(&cell, true);
		column.add_attribute(&cell, "text", Columns::Text as i32);
		left_tree.append_column(&column);
	}

    for i in 0..10 {
        // insert_with_values takes two slices: column indices and ToValue
        // trait objects. ToValue is implemented for strings, numeric types,
        // bool and Object descendants
        let iter = left_store.insert_with_values(None, None, &[Columns::Active as u32, Columns::Text as u32], &[&true, &format!("Helffffffffffflo {}", i)]);

        for _ in 0..i {
            left_store.insert_with_values(Some(&iter), None, &[Columns::Active as u32, Columns::Text as u32], &[&true, &"I"]);
        }
    }

    let button = gtk::Button::new_with_label("Click me!");
	
	let split_pane = gtk::Box::new(Orientation::Horizontal, 10);

    split_pane.set_size_request(-1, -1);
	split_pane.add(&left_tree);
    //split_pane.add(&button);
	let scrolled_window = gtk::ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
	scrolled_window.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
	//scrolled_window.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
	let listbox = gtk::ListBox::new();
	for entry in log_entries {
		let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 5);
		let label = gtk::Label::new(Some(&entry.message));
		label.set_selectable(true);
		//let button = gtk::Button::new_with_label("Click me!");
		let list_box_row = gtk::ListBoxRow::new();
		hbox.add(&label);
		//hbox.add(&button);
		list_box_row.add(&hbox);
		listbox.add(&list_box_row);
	}
	scrolled_window.add(&listbox);
	split_pane.pack_end(&scrolled_window, true, true, 10);
	//split_pane.add(&scrolled_window);

    window.add(&split_pane);
    window.show_all();
}

fn main() {
    let application =
        gtk::Application::new(Some("com.github.BenjaminRi.Sherlog"), Default::default())
            .expect("Initialization failed...");

    application.connect_activate(|app| {
        build_ui(app);
    });

    application.run(&args().collect::<Vec<_>>());
}

/*
DateTime utilities:
let dt = Utc.ymd(2018, 1, 26).and_hms_micro(18, 30, 9, 453_829);
println!("{}", dt.to_rfc3339_opts(SecondsFormat::Millis, false));
let ts_milli : u64 = 1568208334469;
let ts_sec   : u64 = ts_milli / 1000;
let ts_nano  : u32 = ((ts_milli - ts_sec * 1000) * 1000_000) as u32;
let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp_opt(ts_sec as i64, ts_nano).expect("Invalid timestamp encounteres"), Utc);
println!("{}", dt.to_rfc3339_opts(SecondsFormat::Millis, false));
let dt : DateTime::<Utc> = DateTime::<FixedOffset>::parse_from_rfc3339("1996-12-19T16:39:57-08:00").expect("Parse error!").with_timezone(&Utc);
println!("{}", dt.to_rfc3339_opts(SecondsFormat::Millis, false));*/


//Old Parser code
/*let mut offset : usize = 0;
for c in line_str.chars() {
	match parser {
		GlogParserState::PreSection => {
			if c == '[' {
				parser = GlogParserState::SectionKind;
			} else {
				parser = GlogParserState::Invalid;
			}
		},
		GlogParserState::SectionKind => (),
		GlogParserState::SectionValue => { parser = GlogParserState::SectionKind; },
		GlogParserState::Invalid => { break; },
	};
	if i == 1 {
		// println!("?{}?", c);
	}
	offset += c.len_utf8()
}*/

/*match &parser {
	GlogParserState::PreSection => {
		if idx.1 == '[' {
			parser = GlogParserState::SectionKind(idx.0 + 1);
		} else {
			parser = GlogParserState::Invalid;
		}
	},
	GlogParserState::SectionKind(kind_offset) => {
		if idx.1 == '|' {
			let kind_str = &line_str[*kind_offset..idx.0];
			println!("{}", &kind_str);
			let kind = match kind_str {
				"tq" => GlogSectionKind::TimestampUs,
				"s" => GlogSectionKind::Severity,
				"m" => GlogSectionKind::Message,
				_ => GlogSectionKind::Unknown,
			};
			parser = GlogParserState::SectionValue(kind, idx.0 + 1);
		}
	},
	GlogParserState::SectionValue(kind, value_offset) => {
		if idx.1 == ']' {
			parser = GlogParserState::SectionValuePost(*kind, *value_offset);
		}
	},
	GlogParserState::SectionValuePost(kind, value_offset) => {
		if idx.1 == ':' {
			//Add field to log entry
			let value_str = &line_str[*value_offset..idx.0-1];
			 println!("Kind: {:?}, Value: {}", kind, value_str);
			parser = GlogParserState::PreSection;
		} else {
			parser = GlogParserState::SectionValue(*kind, *value_offset);
		}
	},
	GlogParserState::Invalid => { break; },
};*/

/*
enum Severity
{
  SEV_CRITICAL      = 0,
  SEV_HARDWARE      = 1,
  SEV_ERROR         = 2,
  SEV_WARNING       = 3,
  SEV_INFO          = 4,
  SEV_NONE          = 5,
}; 

enum Severity
{
  OFF = SLOG2_SHUTDOWN - 1,    ///filter setting only: no log passes.
  SHUTDOWN = SLOG2_SHUTDOWN,   ///sporadic abnormal event, whole system corrupted, e.g.  (most severe)
  CRITICAL = SLOG2_CRITICAL,   ///sporadic abnormal event, functionality corrupted, e.g.
  ERROR = SLOG2_ERROR,         ///sporadic abnormal event, functionality immediately affected, e.g.
  WARNING = SLOG2_WARNING,     ///sporadic abnormal event, functionality not immediately affected, e.g.
  NOTICE = SLOG2_NOTICE,       ///sporadic data, business content, e.g.
  INFO = SLOG2_INFO,           ///sporadic data, generic content, e.g.
  DEBUG_NORMAL = SLOG2_DEBUG1, ///low bandwidth debug data, business content, e.g.
  DEBUG_DETAIL = SLOG2_DEBUG2, ///high bandwidth debug data, generic content, e.g. all communication accesses (least severe)
  DEFAULT,                     ///filter setting only: set the initial severity.
};
*/
