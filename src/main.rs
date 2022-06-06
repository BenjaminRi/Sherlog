//Hide Windows cmd console on opening the application
//#![windows_subsystem = "windows"]
#![allow(dead_code)]
#![allow(unused_imports)]

use gio::{ListModel, ListStore};
use gtk::prelude::*;
#[allow(unused_imports)]
use gtk::{gdk, gio, glib};
#[allow(unused_imports)]
use gtk::{
	Align, Application, ApplicationWindow, Box, Button, CellRendererText, EventControllerScroll,
	EventControllerScrollFlags, Frame, Label, ListView, MessageDialog, Notebook, Orientation,
	ScrolledWindow, Spinner, TreeListModel, TreeStore, TreeView, Widget,
};

use glib::{clone, MainContext, PRIORITY_DEFAULT};

use std::thread;
use std::time::{Duration, Instant};

use std::cell::RefCell;
use std::rc::Rc;

mod log_store;
mod model;
mod model_internal;
mod parse;

use parse::io::LogParseError;

use log_store::LogStoreLinear;
use log_store::ScrollBarVert;

use model_internal::LogEntryExt;
use model_internal::LogSourceContentsExt;
use model_internal::LogSourceExt;

fn gio_files_to_paths(gio_files: &[gio::File]) -> Vec<std::path::PathBuf> {
	let mut result = Vec::new();
	for gio_file in gio_files {
		result.push(gio_file.path().expect("Invalid file path"));
	}
	result
}

fn about_dialog(window: &gtk::ApplicationWindow, app: &gtk::Application) -> gtk::AboutDialog {
	gtk::AboutDialog::builder()
		.name("Sherlog")
		.version(env!("CARGO_PKG_VERSION"))
		.website_label("Website")
		.website(env!("CARGO_PKG_REPOSITORY"))
		.comments(env!("CARGO_PKG_DESCRIPTION"))
		.license_type(gtk::License::Gpl30Only)
		.copyright("Copyright © 2019-2022 Benjamin Richner")
		.authors(vec![env!("CARGO_PKG_AUTHORS").to_string()])
		.transient_for(window)
		.application(app)
		.modal(true)
		//.logo_icon_name("TODO")
		.build()
}

fn generate_test_log() -> model::LogSource {
	let log_entries = vec![
		model::LogEntry {
			message: "TestCritical 121343245345".to_string(),
			severity: model::LogLevel::Critical,
			..Default::default()
		},
		model::LogEntry {
			message: "TestError 3405834068".to_string(),
			severity: model::LogLevel::Error,
			..Default::default()
		},
		model::LogEntry {
			message: "TestWarning 340958349068".to_string(),
			severity: model::LogLevel::Warning,
			..Default::default()
		},
		model::LogEntry {
			message: "TestInfo 3049580349568".to_string(),
			severity: model::LogLevel::Info,
			..Default::default()
		},
		model::LogEntry {
			message: "TestDebug 0345986045968".to_string(),
			severity: model::LogLevel::Debug,
			..Default::default()
		},
		model::LogEntry {
			message: "TestTrace 309468456".to_string(),
			severity: model::LogLevel::Trace,
			..Default::default()
		},
	];

	let log_source_ex = model::LogSource {
		name: "example".to_string(),
		children: { model::LogSourceContents::Entries(log_entries) },
	};
	let log_source_ex2_1 = model::LogSource {
		name: "example2_1".to_string(),
		children: { model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) },
	};
	let log_source_ex2 = model::LogSource {
		name: "example2".to_string(),
		children: { model::LogSourceContents::Sources(vec![log_source_ex2_1]) },
	};
	let log_source_ex3 = model::LogSource {
		name: "example3".to_string(),
		children: { model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) },
	};
	let log_source_ex4_1 = model::LogSource {
		name: "example4_1".to_string(),
		children: { model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) },
	};
	let log_source_ex4_2 = model::LogSource {
		name: "example4_2".to_string(),
		children: { model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) },
	};
	let log_source_ex4 = model::LogSource {
		name: "example4".to_string(),
		children: { model::LogSourceContents::Sources(vec![log_source_ex4_1, log_source_ex4_2]) },
	};

	model::LogSource {
		name: "Root LogSource".to_string(),
		children: {
			model::LogSourceContents::Sources(vec![
				log_source_ex,
				log_source_ex2,
				log_source_ex3,
				log_source_ex4,
			])
		},
	}
}

struct GlobalGuiModel {
	gui_model: Rc<RefCell<GuiModel>>,
}

impl GlobalGuiModel {
	fn new(gui_model: Rc<RefCell<GuiModel>>) -> GlobalGuiModel {
		GlobalGuiModel { gui_model }
	}
}

pub type Pageid = u32;

struct GuiPage {
	id: Pageid,
	widget: Widget,
}

struct GuiModel {
	notebook: Notebook,
	next_page_id: Pageid,
	pages: Vec<GuiPage>,
}

fn add_scoll_workaround(scroll: &ScrolledWindow) {
	// Workaround for bug https://gitlab.gnome.org/GNOME/gtk/-/issues/2971

	let controller = EventControllerScroll::builder()
		.flags(EventControllerScrollFlags::VERTICAL)
		.build();

	let scroll_clone = scroll.clone();
	let last_frame_counter = RefCell::new(0);
	controller.connect_scroll(move |_, _dx, _dy| {
		let mut last_frame_counter = last_frame_counter.borrow_mut();
		let new_frame_counter = scroll_clone.frame_clock().unwrap().frame_counter();
		/*
		let now = Instant::now();
		println!(
			"Scroll: {:?} {}, {} ({:?}) ({:?})",
			now,
			_dx,
			_dy,
			new_frame_counter,
			scroll_clone.vadjustment().value()
		);
		*/
		if *last_frame_counter == new_frame_counter {
			//println!("Inhibited!");
			gtk::Inhibit(true)
		} else {
			*last_frame_counter = new_frame_counter;
			gtk::Inhibit(false)
		}
	});

	scroll.add_controller(&controller);
}

impl GuiModel {
	fn new(notebook: Notebook) -> GuiModel {
		GuiModel {
			notebook,
			next_page_id: 1,
			pages: Vec::<GuiPage>::new(),
		}
	}
	fn add_page(&mut self) -> Pageid {
		let gtkbox = Box::builder()
			.halign(Align::Center)
			.valign(Align::Center)
			.build();

		let spinner = Spinner::builder()
			.width_request(50)
			.height_request(50)
			.vexpand(false)
			.hexpand(false)
			.halign(Align::Center)
			.valign(Align::Center)
			.build();
		spinner.start();

		gtkbox.append(&spinner);

		let label = Label::new(Some("GuiModel created"));
		let id = self.next_page_id;
		self.next_page_id = self
			.next_page_id
			.checked_add(1)
			.expect("Too many tabs, id counter overflow");

		self.pages.push(GuiPage {
			id,
			widget: gtkbox.clone().upcast::<Widget>(),
		});

		self.notebook.append_page(&gtkbox, Some(&label));
		self.notebook.set_tab_reorderable(&gtkbox, true);

		id
	}
	fn remove_page(&mut self, id: Pageid) {
		for page in &self.pages {
			if page.id == id {
				self.notebook.remove_page(Some(
					self.notebook
						.page_num(&page.widget)
						.expect("Bug in page managing code"),
				));
			}
		}
	}
	fn populate_page(&mut self, id: Pageid, parse_result: Result<model::LogSource, LogParseError>) {
		for page in &self.pages {
			if page.id == id {
				let gtkbox = page
					.widget
					.clone()
					.downcast::<Box>()
					.expect("Bug in GTK page handling");
				gtkbox.remove(&gtkbox.first_child().expect("Bug in GTK box"));
				gtkbox.set_halign(Align::Fill);
				gtkbox.set_valign(Align::Fill);

				/*let sl = gtk::StringList::new(&[]);
				for x in 0..290 {
					//5_000_000
					sl.append(&format!("String{}", x)); //GtkStringObject
				}*/
				let sl = ListStore::new(glib::Type::OBJECT);
				for x in 0..290 {
					sl.append(&gtk::StringObject::new(&format!("String{}", x)));
				}

				let slif = gtk::SignalListItemFactory::new();

				slif.connect_setup(|slif, list_item| {
					let b = gtk::Button::new();
					let expander = gtk::TreeExpander::new();
					expander.set_child(Some(&b));
					list_item.set_child(Some(&expander));
				});

				slif.connect_bind(move |slif, list_item| {
					//println!("{:?}", list_item.item().unwrap().type_());
					let row = list_item
						.item()
						.unwrap()
						.downcast::<gtk::TreeListRow>()
						.unwrap();
					let row_expander = list_item
						.child()
						.unwrap()
						.downcast::<gtk::TreeExpander>()
						.unwrap();
					row_expander.set_list_row(Some(&row));

					let string = list_item
						.item()
						.unwrap()
						.downcast::<gtk::TreeListRow>()
						.unwrap()
						.item()
						.unwrap()
						.downcast::<gtk::StringObject>()
						.unwrap()
						.string();

					list_item
						.child()
						.unwrap()
						.downcast::<gtk::TreeExpander>()
						.unwrap()
						.child()
						.unwrap()
						.downcast::<gtk::Button>()
						.unwrap()
						.set_label(string.as_str());
				});

				let tlm = TreeListModel::new(&sl, false, false, |list_item| {
					let s2 = gtk::StringList::new(&[]);
					println!(
						"Create model {}",
						list_item
							.clone()
							.downcast::<gtk::StringObject>()
							.unwrap()
							.string()
					);
					for x in 0..100 {
						s2.append(&format!("AAA{}", x));
					}
					Some(s2.upcast::<ListModel>())
					//None
				});

				let s = gtk::NoSelection::new(Some(&tlm)); //SingleSelection, NoSelection, MultiSelection

				let columnview = gtk::ColumnView::builder().model(&s).build();
				let column = gtk::ColumnViewColumn::builder()
					.title("Test")
					.factory(&slif)
					.build();
				columnview.append_column(&column);

				//let listview = ListView::builder().model(&s).factory(&slif).build();

				let scroll = ScrolledWindow::builder()
					.child(&columnview)
					.overlay_scrolling(false)
					.min_content_width(200)
					.build();

				add_scoll_workaround(&scroll);
				gtkbox.append(&scroll);

				let label = Label::new(Some("Populating"));
				gtkbox.append(&label);
			}
		}
	}
}

/*Scroll: Instant { t: 6884.4772108s } 0, 1 (1050) (196325.5478039555)
Inhibited!
Scroll: Instant { t: 6884.4799927s } 0, 1 (1050) (196325.5478039555)
Inhibited!
Scroll: Instant { t: 6884.4829862s } 0, 1 (1050) (196325.5478039555)
Inhibited!
Scroll: Instant { t: 6884.4915749s } 0, 1 (1051) (196325.0)
Scroll: Instant { t: 6884.4957788s } 0, 1 (1051) (196380.5478039555)
Inhibited!
Scroll: Instant { t: 6884.4999516s } 0, 1 (1051) (196380.5478039555)
Inhibited!
Scroll: Instant { t: 6884.506177s } 0, 1 (1052) (196380.0)
Scroll: Instant { t: 6884.5116088s } 0, 1 (1052) (196435.5478039555)
Inhibited!

(sherlog.exe:16812): Gtk-WARNING **: 20:26:23.725: GtkListView failed to scroll to given position. Ignoring...
Scroll: Instant { t: 6884.7352692s } 0, 1 (1065) (196808.0)
Scroll: Instant { t: 6884.755972s } 0, 1 (1067) (196863.0)
Scroll: Instant { t: 6884.7584715s } 0, 1 (1067) (196918.5478039555)
Inhibited!
Scroll: Instant { t: 6884.7598294s } 0, 1 (1067) (196918.5478039555)
Inhibited!
Scroll: Instant { t: 6884.7642249s } 0, 1 (1067) (196918.5478039555)
Inhibited!
Scroll: Instant { t: 6884.7662624s } 0, 1 (1067) (196918.5478039555)
Inhibited!
Scroll: Instant { t: 6884.7702319s } 0, 1 (1067) (196918.5478039555)
Inhibited!
Scroll: Instant { t: 6884.7720778s } 0, 1 (1067) (196918.5478039555)
Inhibited!
Scroll: Instant { t: 6884.7760662s } 0, 1 (1068) (196918.0)
Scroll: Instant { t: 6884.7788259s } 0, 1 (1068) (196973.5478039555)
Inhibited!
Scroll: Instant { t: 6884.7828175s } 0, 1 (1068) (196973.5478039555)
Inhibited!
Scroll: Instant { t: 6884.7858359s } 0, 1 (1068) (196973.5478039555)
Inhibited!
Scroll: Instant { t: 6884.7940174s } 0, 1 (1069) (196973.0)
Scroll: Instant { t: 6884.7955547s } 0, 1 (1069) (197028.5478039555)
Inhibited!
Scroll: Instant { t: 6884.8012353s } 0, 1 (1069) (197028.5478039555)*/

fn main() {
	fern::Dispatch::new()
		// Perform allocation-free log formatting
		.format(|out, message, record| {
			out.finish(format_args!(
				"{}[{}][{}] {}",
				chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
				record.target(),
				record.level(),
				message
			))
		})
		.level(log::LevelFilter::Warn)
		.level_for("sherlog", log::LevelFilter::Trace)
		.chain(std::io::stdout())
		//.chain(fern::log_file("output.log").unwrap())
		// Apply globally
		.apply()
		.unwrap();

	// How to select/force theme:
	//std::env::set_var("GTK_THEME", "Aero");

	// Activate GTK inspector:
	// SET GTK_DEBUG=interactive

	let mut flags = gio::ApplicationFlags::empty();
	//flags.insert(gio::ApplicationFlags::NON_UNIQUE);
	flags.insert(gio::ApplicationFlags::HANDLES_OPEN);
	let app = Application::builder()
		.application_id("com.github.BenjaminRi.Sherlog")
		.flags(flags)
		.build();

	let global_gui_model = Rc::new(RefCell::<Option<GlobalGuiModel>>::new(None));

	{
		let global_gui_model = global_gui_model.clone();
		app.connect_open(move |app, gio_files, _| {
			println!("connect_open");
			build_ui(
				app,
				&gio_files_to_paths(gio_files),
				&mut global_gui_model.borrow_mut(),
			);
		});
	}

	{
		let global_gui_model = global_gui_model.clone();
		app.connect_activate(move |app| {
			println!("connect_activate");
			build_ui(app, &Vec::new(), &mut global_gui_model.borrow_mut());
		});
	}

	//Without NON_UNIQUE:
	//Process 1
	//connect_startup
	//connect_activate
	//connect_activate
	//Process 2 (quits immediately)
	//
	// With NON_UNIQUE:
	//Process 1
	//connect_startup
	//connect_activate
	//Process 2
	//connect_startup
	//connect_activate

	app.run();
}

fn apply_hardcoded_stylesheet() {
	// This is required for Windows, to fix the titlebar look and feel
	let provider = gtk::CssProvider::new();
	provider.load_from_data(include_bytes!("style.css"));
	gtk::StyleContext::add_provider_for_display(
		&gtk::gdk::Display::default().expect("Could not connect to a display."),
		&provider,
		gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
	);
}

fn build_ui(
	app: &gtk::Application,
	file_paths: &[std::path::PathBuf],
	global_gui_model: &mut std::option::Option<GlobalGuiModel>,
) {
	apply_hardcoded_stylesheet();

	let mut window = ApplicationWindow::builder()
		.application(app)
		.title("Sherlog")
		.default_width(640)
		.default_height(480)
		.build();

	{
		let about = gio::SimpleAction::new("about", None);
		let window_c = window.clone();
		let app_c = app.clone();
		about.connect_activate(move |_, _| {
			let d = about_dialog(&window_c, &app_c);
			d.present();
		});
		app.add_action(&about);
	}

	let notebook = Notebook::builder().show_border(false).build();
	let gui_model = Rc::new(RefCell::new(GuiModel::new(notebook.clone())));
	*global_gui_model = Some(GlobalGuiModel::new(gui_model.clone()));

	let (sender, receiver) = MainContext::channel(PRIORITY_DEFAULT);

	{
		let gui_model = gui_model.clone();
		receiver.attach(None, move |(page_id, parse_result)| {
			log::info!("Parsing done signal");
			let mut gui_model = gui_model.borrow_mut();
			gui_model.populate_page(page_id, parse_result);
			Continue(true)
		});
	}

	//let mut dialog_vec: Vec<MessageDialog> = Vec::<MessageDialog>::new();
	/*dialog_vec.push(gtk::MessageDialog::new(
		Some(&window),
		gtk::DialogFlags::empty(),
		gtk::MessageType::Error,
		gtk::ButtonsType::Ok,
		&error_str,
	));*/

	log::info!("File paths: {:?}", file_paths);
	if !file_paths.is_empty() {
		// We are handling an open signal and need to open and read files

		for file_path in file_paths {
			let mut gui_model = gui_model.borrow_mut();

			// Add a loading page to the GUI, it will be filled with
			// content after parsing is done
			let page_id = gui_model.add_page();

			let file_path = file_path.clone();
			let sender = sender.clone();

			thread::spawn(move || {
				let now = Instant::now();

				// Parsing is potentially long running, this is why
				// it is running in a separate thread here
				let parse_result = parse::from_file(&file_path);

				let elapsed = now.elapsed();
				log::info!(
					"Time to parse file [{:?}]: {}ms",
					&file_path,
					elapsed.as_secs() * 1000 + elapsed.subsec_millis() as u64
				);

				thread::sleep(Duration::from_secs(1));
				sender
					.send((page_id, parse_result))
					.expect("Could not send through channel");
			});
		}
	}

	// Present window
	window.set_child(Some(&notebook));
	window.present();
	println!("end of func");
}

/*
SET GTK_DEBUG=interactive

------------

.default-decoration.titlebar:not(headerbar), headerbar.default-decoration { min-height: 28px; padding: 4px; }

button.close { margin-left: 1px; margin-right: 6px; min-height: 13px; padding-top: 0px; padding-bottom: 0px; padding-left: 5px; padding-right: 5px; border-radius: 0px; color: white; -gtk-icon-shadow: 0 1px #707070, 1px 0px #707070, 0 -1px #707070, -1px 0 #707070; }

-------------

window.csd { box-shadow: 5px 0px 0px 0px #707070, 0px 5px 0px 0px #707070, 0px 0px 5px 0px #707070, 0px 0px 0px 5px #707070, 4px 0px 0px 0px #b5cee7, 0px 4px 0px 0px #b5cee7, 0px 0px 4px 0px #b5cee7, 0px 0px 0px 4px #b5cee7; margin: 0px; border-radius: 0px 0px 0 0; }

-------------



/*
You can type here any CSS rule recognized by GTK.
You can temporarily disable this custom CSS by clicking on the “Pause” button above.

Changes are applied instantly and globally, for the whole application.
*/

.default-decoration.titlebar:not(headerbar), headerbar.default-decoration { padding-right: 0px; padding-top: 0px; }

button.close { border-radius: 0px; }


---------------------------

.default-decoration.titlebar:not(headerbar), headerbar.default-decoration { padding-right: 0px; padding-top: 0px; }

button.close { border-radius: 0px; padding-top: 4px; padding-right: 4px; }
button.minimize { border-radius: 0px; padding-top: 4px; }
button.maximize { border-radius: 0px; padding-top: 4px; }


*/

/*
extern crate chrono;
extern crate gtk;
extern crate log;

use gtk::cairo;
use gtk::gdk;
use gtk::gio;
use gtk::glib;

use gdk::EventMask;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::DrawingArea;

use std::time::Instant;

use std::cell::RefCell;
use std::rc::Rc;

#[allow(unused_imports)]
use regex::Regex;

mod log_store;
mod model;
mod model_internal;
mod parse;

use log_store::LogStoreLinear;
use log_store::ScrollBarVert;

use model_internal::LogEntryExt;
use model_internal::LogSourceContentsExt;
use model_internal::LogSourceExt;

#[allow(unused_imports)]
use gtk::{
	ApplicationWindow, ButtonsType, CellRendererPixbuf, CellRendererText, DialogFlags, ListStore,
	MessageDialog, MessageType, Orientation, TreeStore, TreeView, TreeViewColumn, WindowPosition,
};

use std::env::args;

enum LogSourcesColumns {
	Active = 0,
	Inconsistent = 1,
	Text = 2,
	Id = 3,
	ChildCount = 4,
}

fn toggle_row(
	tree_store: &gtk::TreeStore,
	store: &mut LogStoreLinear,
	drawing_area: &gtk::DrawingArea,
	path: gtk::TreePath,
) {
	//log::info!("Path: {:?}", path.get_indices());

	let iter = tree_store.iter(&path).unwrap();
	let mut active = tree_store
		.value(&iter, LogSourcesColumns::Active as i32)
		.get::<bool>()
		.unwrap();
	let mut inconsistent = tree_store
		.value(&iter, LogSourcesColumns::Inconsistent as i32)
		.get::<bool>()
		.unwrap();

	if inconsistent || !active {
		inconsistent = false;
		active = true;
	} else {
		active = false;
	}

	tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &active.to_value());
	tree_store.set_value(
		&iter,
		LogSourcesColumns::Inconsistent as u32,
		&inconsistent.to_value(),
	);

	fn check_inconsistent(tree_store: &gtk::TreeStore, mut path: gtk::TreePath) -> bool {
		let mut prev_active = None;
		if path.up() {
			path.append_index(0);
			while let Some(iter) = tree_store.iter(&path) {
				let n_active = tree_store
					.value(&iter, LogSourcesColumns::Active as i32)
					.get::<bool>()
					.unwrap();

				let n_inconsistent = tree_store
					.value(&iter, LogSourcesColumns::Inconsistent as i32)
					.get::<bool>()
					.unwrap();

				if (prev_active != None && Some(n_active) != prev_active) || n_inconsistent {
					return true;
				}
				prev_active = Some(n_active);
				path.next();
			}
		}
		false
	}

	{
		#[allow(clippy::redundant_clone)]
		let mut path_up = path.clone();
		let mut level_inconsistent = check_inconsistent(tree_store, path_up.clone());
		while path_up.up() && path_up.depth() > 0 {
			let iter = tree_store.iter(&path_up).unwrap();
			if level_inconsistent {
				tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &false.to_value());
			} else {
				tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &active.to_value());
			}
			tree_store.set_value(
				&iter,
				LogSourcesColumns::Inconsistent as u32,
				&level_inconsistent.to_value(),
			);
			level_inconsistent = check_inconsistent(tree_store, path_up.clone());
		}
	}

	fn activate_children(
		tree_store: &gtk::TreeStore,
		iter: &gtk::TreeIter,
		active: bool,
		sources: &mut Vec<u32>,
	) {
		if let Some(iter) = tree_store.iter_children(Some(iter)) {
			loop {
				/*let n_text = tree_store
					.value(&iter, LogSourcesColumns::Text as i32)
					.get::<String>()
					.unwrap()
					.unwrap();

				log::info!("{}", n_text);*/
				let n_active = tree_store
					.value(&iter, LogSourcesColumns::Active as i32)
					.get::<bool>()
					.unwrap();
				if n_active != active {
					tree_store.set_value(
						&iter,
						LogSourcesColumns::Active as u32,
						&active.to_value(),
					);
				}
				let n_id = tree_store
					.value(&iter, LogSourcesColumns::Id as i32)
					.get::<u32>()
					.unwrap();
				//log::info!("activate_children... {}", n_id);
				sources.push(n_id); //Don't just push diffs. Push continuous ranges to enable optimization below.
				tree_store.set_value(
					&iter,
					LogSourcesColumns::Inconsistent as u32,
					&false.to_value(),
				);

				activate_children(tree_store, &iter, active, sources);
				if !tree_store.iter_next(&iter) {
					break;
				}
			}
		}
	}
	let mut sources = Vec::<u32>::new();
	let id = tree_store
		.value(&iter, LogSourcesColumns::Id as i32)
		.get::<u32>()
		.unwrap();
	sources.push(id);

	let now = Instant::now();
	activate_children(tree_store, &iter, active, &mut sources);
	let elapsed = now.elapsed();
	log::info!(
		"Time to activate children: {}ms",
		elapsed.as_secs() * 1000 + elapsed.subsec_millis() as u64
	);

	//log::info!("Click: {:?} change to {}", sources, active); //Note: Very verbose output.

	let mut ordered = true;
	let mut next_id = *sources.first().unwrap(); //We can do this because we know we pushed at least one id above.
	for id in sources.iter() {
		if next_id != *id {
			ordered = false;
			break;
		} else {
			next_id += 1;
		}
	}

	if !ordered {
		log::error!("Unordered log source tree detected!");
		panic!(); //If this happens you broke the tree structure
	}

	let first_id = *sources.first().unwrap(); //We can do this because we know we pushed at least one id above.
	let last_id = *sources.last().unwrap(); //We can do this because we know we pushed at least one id above.

	//log::info!("Click: Range [{},{}] set to {}", first_id, last_id, active);
	//log::info!("Inconsistent: {}", level_inconsistent);

	/*
	let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
	assert!(re.is_match("2014-01-01"));
	let re = Regex::new(r"job").unwrap();
	re.is_match(&entry.message)
	*/

	let now = Instant::now();
	store.filter_store(
		&|entry: &LogEntryExt| entry.source_id >= first_id && entry.source_id <= last_id,
		active,
		crate::model_internal::VISIBLE_OFF_SOURCE,
	);
	let elapsed = now.elapsed();
	log::info!(
		"Time to update store: {}ms",
		elapsed.as_secs() * 1000 + elapsed.subsec_millis() as u64
	);

	drawing_area.queue_draw();
}

//--------------------------------------------------------------------------------------------------
//--------------------------------------------------------------------------------------------------
//--------------------------------------------------------------------------------------------------
//--------------------------------------------------------------------------------------------------

fn draw(
	store: &mut LogStoreLinear,
	drawing_area: &DrawingArea,
	ctx: &cairo::Context,
) -> gtk::Inhibit {
	//store.store.push(model::LogEntry { message: "TestTrace 309468456".to_string(),       severity: model::LogLevel::Trace,    ..Default::default() });
	//log::info!("{}", store.store.len());

	//log::info!("w: {} h: {}", drawing_area.allocated_width(), drawing_area.allocated_height());

	ctx.set_source_rgb(1.0, 1.0, 1.0);
	ctx.paint();

	/*ctx.set_source_rgb(1.0, 0.0, 0.0);
	ctx.rectangle(10.0, 10.0, 2.0, 2.0);
	ctx.fill();

	ctx.set_source_rgb(0.0, 0.0, 0.0);
	ctx.new_sub_path();
	ctx.move_to(10.0, 10.0);
	ctx.line_to(10.0, 20.0);
	ctx.line_to(20.0, 20.0);
	ctx.line_to(20.0, 10.0);
	ctx.close_path();
	ctx.stroke();*/

	let h = drawing_area.allocated_height();
	let w = drawing_area.allocated_width();

	ctx.set_source_rgb(0.0, 0.0, 0.0);

	store.line_spacing = f64::max(store.line_spacing, store.font_size + 2.0); //prevent overlapping lines with large font
	store.visible_lines = (f64::max(0.0, (h as f64) - store.border_top - store.border_bottom)
		/ store.line_spacing) as usize;

	if store.store.len() < store.visible_lines {
		//No scrolling possible, less entries than rows on GUI!
		store.viewport_offset = 0;
	} else if store.viewport_offset > store.store.len() - store.visible_lines {
		store.viewport_offset = store.store.len() - store.visible_lines;
	}

	//-----------------------------------------------------------------------------
	//Draw loop
	//-----------------------------------------------------------------------------
	let mut anchor_drawn = false;

	for (i, (offset, entry)) in store
		.store
		.iter()
		.enumerate() //offset in vector
		.skip(store.viewport_offset)
		.filter(|(_, x)| x.is_visible())
		.take(store.visible_lines)
		//index of filtered element:
		.enumerate()
	{
		ctx.select_font_face(
			"Lucida Console", //"Calibri"
			cairo::FontSlant::Normal,
			cairo::FontWeight::Normal,
		);
		ctx.set_font_size(store.font_size);

		let mut draw_highlight = if Some(i) == store.hover_line {
			ctx.set_source_rgb(0.8, 0.8, 0.8);
			true
		} else {
			false
		};

		if (store.selected_single.contains(&offset)
			|| (store.selected_range.is_some()
				&& store.selected_range.unwrap().0 <= offset
				&& store.selected_range.unwrap().1 >= offset))
			&& !store.excluded_single.contains(&offset)
		{
			if draw_highlight {
				//Cumulative: Row is selected and hovered over
				ctx.set_source_rgb(0.7, 0.7, 1.0);
			} else {
				ctx.set_source_rgb(0.8, 0.8, 1.0);
			}
			draw_highlight = true;
		}

		if draw_highlight {
			ctx.rectangle(
				0.0,
				store.border_top + store.line_spacing * i as f64,
				w as f64,
				store.line_spacing,
			);
			ctx.fill();
		}

		match entry.severity {
			model::LogLevel::Critical => {
				ctx.set_source_rgb(0.5, 0.0, 0.0);
			} // Dark red
			model::LogLevel::Error => {
				ctx.set_source_rgb(1.0, 0.0, 0.0);
			} //Red
			model::LogLevel::Warning => {
				ctx.set_source_rgb(0.77, 0.58, 0.0);
			} //Dirty yellow-orange
			model::LogLevel::Info => {
				ctx.set_source_rgb(0.0, 0.0, 0.0);
			} //Black
			model::LogLevel::Debug => {
				ctx.set_source_rgb(0.6, 0.6, 0.6);
			} //Grey
			model::LogLevel::Trace => {
				ctx.set_source_rgb(0.4, 0.4, 0.4);
			} //Light grey
		}

		let offset_y = store.border_top
			+ store.line_spacing * i as f64
			+ f64::max(0.0, store.line_spacing - store.font_size) / 2.0;
		//Anchor point of text is bottom left, excluding descent.
		//We want to anchor on top left though, so calculate that away:
		let font_offset_y = offset_y + store.font_size - ctx.font_extents().unwrap().descent;

		let date_str = entry.timestamp.format("%d.%m.%y %T%.3f").to_string();
		ctx.move_to(store.border_left, font_offset_y);
		ctx.show_text(&date_str);

		let short_sev = match entry.severity {
			model::LogLevel::Critical => "CRI",
			model::LogLevel::Error => "ERR",
			model::LogLevel::Warning => "WRN",
			model::LogLevel::Info => "INF",
			model::LogLevel::Debug => "DBG",
			model::LogLevel::Trace => "TRC",
		};

		ctx.move_to(store.border_left + 180.0, font_offset_y);
		ctx.show_text(&short_sev);

		if let Some(anchor_offset) = store.anchor_offset {
			if offset == anchor_offset {
				ctx.move_to(store.border_left - 20.0, font_offset_y);
				ctx.show_text(&"→"); //TODO: Replace with anchor symbol
				anchor_drawn = true;
			} else if !anchor_drawn {
				if offset >= anchor_offset {
					ctx.move_to(
						store.border_left - 20.0,
						font_offset_y - store.line_spacing / 2.0,
					);
					ctx.show_text(&"→"); //TODO: Replace with anchor symbol
					anchor_drawn = true;
				} else if i == store.visible_lines - 1 || offset == store.last_offset {
					ctx.move_to(
						store.border_left - 20.0,
						font_offset_y + store.line_spacing / 2.0,
					);
					ctx.show_text(&"→"); //TODO: Replace with anchor symbol
					anchor_drawn = true;
				}
			}
		}

		ctx.move_to(store.border_left + 210.0, font_offset_y);

		/*let font_face = ctx.get_font_face();
		let new_font_face = cairo::FontFace::toy_create("cairo :monospace", font_face.toy_get_slant(), font_face.toy_get_weight());
		ctx.set_font_face(&new_font_face);*/

		ctx.show_text(&entry.message);

		/*if let Some(source_name) = store.log_sources.get(&entry.source_id) {
			ctx.move_to(store.border_left + 210.0, font_offset_y + store.font_size);
			ctx.set_font_size(f64::round(store.font_size * 0.7));
			ctx.set_source_rgb(0.5, 0.5, 0.5);
			ctx.show_text(source_name);
			ctx.set_font_size(store.font_size);
		}*/
	}

	{
		store.scroll_bar.bar_height = h as f64 - store.scroll_bar.bar_padding * 2.0;

		store.scroll_bar.scroll_perc = store.get_scroll_percentage(store.visible_lines);

		store.scroll_bar.thumb_rel_offset = f64::round(
			(store.scroll_bar.bar_height
				- store.scroll_bar.thumb_height
				- store.scroll_bar.thumb_margin * 2.0)
				* store.scroll_bar.scroll_perc,
		) + store.scroll_bar.thumb_margin;

		store.scroll_bar.x = w as f64 - store.scroll_bar.bar_width - store.scroll_bar.bar_padding;
		store.scroll_bar.y = store.scroll_bar.bar_padding;

		store.scroll_bar.thumb_width =
			store.scroll_bar.bar_width - 2.0 * store.scroll_bar.thumb_margin;
		store.scroll_bar.thumb_x = store.scroll_bar.x + store.scroll_bar.thumb_margin;
		store.scroll_bar.thumb_y = store.scroll_bar.y + store.scroll_bar.thumb_rel_offset;

		ctx.set_source_rgb(0.7, 0.7, 0.7);
		ctx.rectangle(
			store.scroll_bar.x,
			store.scroll_bar.y,
			store.scroll_bar.bar_width,
			store.scroll_bar.bar_height,
		);
		ctx.fill();

		ctx.set_source_rgb(0.3, 0.3, 0.3);
		ctx.rectangle(
			store.scroll_bar.thumb_x,
			store.scroll_bar.thumb_y,
			store.scroll_bar.thumb_width,
			store.scroll_bar.thumb_height,
		);
		ctx.fill();
	}

	gtk::Inhibit(false)
}

fn handle_evt(
	_store: &mut LogStoreLinear,
	_drawing_area: &DrawingArea,
	evt: &gdk::Event,
) -> gtk::Inhibit {
	//_drawing_area.queue_draw();
	if evt.event_type() != gdk::EventType::MotionNotify {
		//Performance test
		/*let mut a = 5.0;
		for entry in _store.store.iter() {
			match entry.severity {
				model::LogLevel::Critical => { a += 1.1; },
				model::LogLevel::Error => { a += 1.23; },
				model::LogLevel::Warning => { a += 1.29; },
				model::LogLevel::Info => { a += 5.984; },
				model::LogLevel::Debug => { a += 6.98; },
				model::LogLevel::Trace => { a += 2.158; },
			}
		}*/
		log::trace!("Event: {:?}", evt.event_type());
	}
	gtk::Inhibit(false)
}

fn handle_evt_scroll(
	store: &mut LogStoreLinear,
	drawing_area: &DrawingArea,
	evt: &gdk::EventScroll,
) -> gtk::Inhibit {
	let scroll_speed = 3;
	let mut dirty = false;
	match evt.direction() {
		gdk::ScrollDirection::Up => {
			dirty = store.scroll(-scroll_speed, store.visible_lines);
		}
		gdk::ScrollDirection::Down => {
			dirty = store.scroll(scroll_speed, store.visible_lines);
		}
		_ => (),
	}

	if dirty {
		drawing_area.queue_draw();
	}

	log::trace!("Scroll... dirty: {}, pos: {}", dirty, store.viewport_offset);

	gtk::Inhibit(false)
}

fn handle_evt_press(
	store: &mut LogStoreLinear,
	drawing_area: &DrawingArea,
	evt: &gdk::EventButton,
) -> gtk::Inhibit {
	//log::info!("PRESS pos:  {:?}", evt.position());
	//log::info!("PRESS root: {:?}", evt.get_root());

	store.mouse_down = true;
	if evt.position().0 >= store.scroll_bar.thumb_x
		&& evt.position().0 <= store.scroll_bar.thumb_x + store.scroll_bar.thumb_width
		&& evt.position().1 >= store.scroll_bar.thumb_y
		&& evt.position().1 <= store.scroll_bar.thumb_y + store.scroll_bar.thumb_height
	{
		store.thumb_drag = true;
		store.thumb_drag_x = evt.position().0 - store.scroll_bar.thumb_x;
		store.thumb_drag_y = evt.position().1 - store.scroll_bar.thumb_y;
		store.hover_line = None;
	} else if !(evt.position().0 < store.border_left || evt.position().1 < store.border_top) {
		let line = ((evt.position().1 - store.border_top) / store.line_spacing) as usize;
		if line < store.visible_lines {
			let clicked_line = store.rel_to_abs_offset(line);

			if let Some(clicked_line_val) = clicked_line {
				if !store.pressed_shift && !store.pressed_ctrl {
					store.selected_single.clear();
					store.excluded_single.clear();
					store.selected_range = None;
					store.selected_single.insert(clicked_line_val);
					store.selected_single_last = clicked_line;
				} else if !store.pressed_shift && store.pressed_ctrl {
					if !store.selected_single.insert(clicked_line_val) {
						store.selected_single.remove(&clicked_line_val);
					}
					if let Some((pivot, clicked_line_old)) = store.selected_range {
						if pivot <= clicked_line_val
							&& clicked_line_old >= clicked_line_val
							&& !store.excluded_single.insert(clicked_line_val)
						{
							store.excluded_single.remove(&clicked_line_val);
						}
					}
					store.selected_single_last = clicked_line;
				} else {
					let pivot = store.selected_single_last.unwrap_or(0);
					if pivot < clicked_line_val {
						store.selected_range = Some((pivot, clicked_line_val));
					} else {
						store.selected_range = Some((clicked_line_val, pivot));
					}
					if store.pressed_shift && store.pressed_ctrl {
						//TODO: 13.04.2020: Behaviour does not reflect 100% how it is in Windows Explorer
						store.selected_single_last = clicked_line;
					} else {
						store.selected_single.clear();
					}
					store.excluded_single.clear();
				}
			} else {
				//Click into the void (no line is there)
				if !store.pressed_shift && !store.pressed_ctrl {
					store.selected_single.clear();
					store.excluded_single.clear();
					store.selected_range = None;
				}
			}

			if clicked_line != store.anchor_offset {
				store.anchor_offset = clicked_line;
				log::info!("SET NEW anchor: {:?}", store.anchor_offset);
				drawing_area.queue_draw();
			}
		}
	}

	gtk::Inhibit(false)
}

fn handle_evt_release(
	store: &mut LogStoreLinear,
	_drawing_area: &DrawingArea,
	_evt: &gdk::EventButton,
) -> gtk::Inhibit {
	//log::info!("RELEASE");
	store.mouse_down = false;
	store.thumb_drag = false;
	store.thumb_drag_x = 0.0;
	store.thumb_drag_y = 0.0;
	gtk::Inhibit(false)
}

fn handle_evt_motion(
	store: &mut LogStoreLinear,
	timediff_entry: &gtk::Entry,
	drawing_area: &DrawingArea,
	evt: &gdk::EventMotion,
) -> gtk::Inhibit {
	if store.thumb_drag {
		store.scroll_bar.thumb_y = evt.position().1 - store.thumb_drag_y;
		store.scroll_bar.thumb_rel_offset = store.scroll_bar.thumb_y - store.scroll_bar.y;
		store.scroll_bar.scroll_perc = (store.scroll_bar.thumb_rel_offset
			- store.scroll_bar.thumb_margin)
			/ (store.scroll_bar.bar_height
				- store.scroll_bar.thumb_height
				- store.scroll_bar.thumb_margin * 2.0);
		if store.scroll_bar.scroll_perc < 0.0 {
			store.scroll_bar.scroll_perc = 0.0;
		} else if store.scroll_bar.scroll_perc > 1.0 {
			store.scroll_bar.scroll_perc = 1.0;
		}
		store.viewport_offset = store
			.percentage_to_offset(store.scroll_bar.scroll_perc, store.visible_lines)
			.unwrap_or(0);
		log::trace!("MOTION {:?}", evt.position());
		drawing_area.queue_draw();
	} else {
		let current_hover = {
			if evt.position().0 < store.border_left || evt.position().1 < store.border_top {
				None
			} else {
				let line = ((evt.position().1 - store.border_top) / store.line_spacing) as usize;
				if line >= store.visible_lines {
					None
				} else {
					Some(line)
				}
			}
		};

		if current_hover != store.hover_line {
			if let Some(line) = current_hover {
				if let Some(hover_entry) = store.rel_to_abs_offset(line) {
					if let Some(anchor) = store.anchor_offset {
						let timediff =
							store.store[hover_entry].timestamp - store.store[anchor].timestamp;
						let mut timediff_ms = i64::abs(timediff.num_milliseconds());
						let days = timediff_ms / 86_400_000;
						timediff_ms -= days * 86_400_000;
						let hours = timediff_ms / 3_600_000;
						timediff_ms -= hours * 3_600_000;
						let minutes = timediff_ms / 60_000;
						timediff_ms -= minutes * 60_000;
						let seconds = timediff_ms / 1000;
						timediff_ms -= seconds * 1000;
						let milliseconds = timediff_ms;
						let sign = if timediff.num_milliseconds() < 0 {
							'-'
						} else {
							'+'
						};
						let text = format!(
							"{}{}D {:02}:{:02}:{:02}.{:03}",
							sign, days, hours, minutes, seconds, milliseconds
						);
						timediff_entry.set_text(&text);
					}
				}
			} else {
				//TODO: This is not perfect... Needs hover to update, even if log store becomes empty:
				timediff_entry.set_text("+0D 00:00:00.000");
			}
			//log::info!("Hover change: {:?}, {:?}", current_hover, store.hover_line);
			store.hover_line = current_hover;
			drawing_area.queue_draw();
		}
	}

	gtk::Inhibit(false)
}

//--------------------------------------------------------------------------------------------------
//--------------------------------------------------------------------------------------------------
//--------------------------------------------------------------------------------------------------

fn build_ui(application: &gtk::Application, file_paths: &[std::path::PathBuf]) {

	// Create log store as Refcounted RefCell to be used in closures ------------------------

	let store = LogStoreLinear {
		store: Vec::<LogEntryExt>::new(),
		entry_count: 0,
		first_offset: 0,
		last_offset: 0,
		anchor_offset: None,

		show_crit: true,
		show_err: true,
		show_warn: true,
		show_info: true,
		show_dbg: true,
		show_trace: true,

		selected_single: std::collections::HashSet::new(),
		excluded_single: std::collections::HashSet::new(),
		selected_single_last: None,
		selected_range: None,

		pressed_shift: false,
		pressed_ctrl: false,

		log_sources: std::collections::HashMap::<u32, String>::new(),

		visible_lines: 0,
		hover_line: None,
		viewport_offset: 0,
		mouse_down: false,
		thumb_drag: false,
		thumb_drag_x: 0.0,
		thumb_drag_y: 0.0,

		border_left: 30.0,
		border_top: 10.0,
		border_bottom: 10.0,
		line_spacing: 20.0,
		font_size: 14.0,

		scroll_bar: ScrollBarVert {
			x: 0.0,
			y: 0.0,

			bar_padding: 10.0,
			bar_width: 20.0,
			bar_height: 0.0, //calculate dynamically

			thumb_x: 0.0, //calculate dynamically
			thumb_y: 0.0, //calculate dynamically
			thumb_margin: 3.0,
			thumb_width: 0.0, //calculate dynamically
			thumb_height: 50.0,
			thumb_rel_offset: 0.0, //calculate dynamically

			scroll_perc: 0.0, //calculate dynamically
		},
	};

	let store_rc = Rc::new(RefCell::new(store));

	//---------------------------------------------------------------------------------------

	let drawing_area = DrawingArea::new();

	let mut log_source_root_ext = LogSourceExt::from_source(log_source_root);

	// left pane
	let left_store = TreeStore::new(&[
		glib::Type::BOOL,
		glib::Type::BOOL,
		String::static_type(),
		glib::Type::U32,
		glib::Type::U64,
	]);
	//let left_store_sort = gtk::TreeModelSort::new(&left_store);
	//Do not use TreeModelSort:
	//https://github.com/gtk-rs/gtk/issues/1000 (closed as problem is not in Rust wrapper)
	//https://gitlab.gnome.org/GNOME/gtk/-/issues/2693 (issue in GTK)
	let sources_tree_view = gtk::TreeView::with_model(&left_store);
	sources_tree_view.set_headers_visible(true);

	/*{
		//https://github.com/ChariotEngine/drs-studio/blob/f0303b52063f0d365732941e5096c42dad06f326/ui/gtk/src/main.rs
		let store_clone = left_store_sort.clone();
		left_store_sort.set_sort_func(
			gtk::SortColumn::Index(LogSourcesColumns::Text as u32),
			move |_w, l_it, r_it| {
				//Crashes. See: https://github.com/gtk-rs/gtk/issues/960
				let l_id = store_clone
					.value(&l_it, LogSourcesColumns::ChildCount as i32)
					.get::<u64>()
					.unwrap();
				let r_id = store_clone
					.value(&r_it, LogSourcesColumns::ChildCount as i32)
					.get::<u64>()
					.unwrap();
				l_id.cmp(&r_id)
			},
		);
	}*/

	// Column with checkbox to toggle log sources, plus log source name
	{
		let column = gtk::TreeViewColumn::new();
		// https://lazka.github.io/pgi-docs/Gtk-3.0/classes/TreeViewColumn.html#Gtk.TreeViewColumn.set_sort_indicator
		column.set_sizing(gtk::TreeViewColumnSizing::Fixed);
		column.set_title("Log source");
		column.set_fixed_width(300);
		//column.set_sort_indicator(true);
		//column.set_clickable(true);
		//column.set_sort_column_id(LogSourcesColumns::Text as i32);

		{
			let renderer_toggle = gtk::CellRendererToggle::new();
			//renderer_toggle.set_property_inconsistent(true);
			renderer_toggle.set_alignment(0.0, 0.0);
			//renderer_toggle.set_padding(0, 0);
			column.pack_start(&renderer_toggle, false);
			column.add_attribute(&renderer_toggle, "active", LogSourcesColumns::Active as i32);
			column.add_attribute(
				&renderer_toggle,
				"inconsistent",
				LogSourcesColumns::Inconsistent as i32,
			);
		}

		{
			let renderer_text = CellRendererText::new();
			gtk::prelude::CellRendererExt::set_alignment(&renderer_text, 0.0, 0.0);
			column.pack_start(&renderer_text, false);
			column.add_attribute(&renderer_text, "text", LogSourcesColumns::Text as i32);
		}

		{
			sources_tree_view.append_column(&column);
			sources_tree_view.set_property("activate-on-single-click", &true);
			//connect_row_activated<F: Fn(&Self, &TreePath, &TreeViewColumn)
			//sources_tree_view.connect_row_activated(|tree_view, path, column| { log::info!("row-activated\n{:?}\n{:?}\n{:?}", tree_view, path.get_indices(), column) } ); //TODO: Hook up row-activated event
			//https://gtk-rs.org/docs/gtk/trait.TreeViewExt.html

			let left_store_clone = left_store.clone(); //GTK objects are refcounted, just clones ref
										   //let model_sort_clone = left_store_sort.clone(); //GTK objects are refcounted, just clones ref
			let drawing_area_clone = drawing_area.clone(); //GTK objects are refcounted, just clones ref
			let store_rc_clone = store_rc.clone();
			sources_tree_view.connect_row_activated(move |_tree_view, path, _column| {
				toggle_row(
					&left_store_clone,
					&mut store_rc_clone.borrow_mut(),
					&drawing_area_clone,
					path.clone(),
				)
			});
		}
	}

	//Column with number of entries of a log source
	{
		let column = gtk::TreeViewColumn::new();
		column.set_title("Entries");
		//column.set_sort_indicator(true);
		//column.set_clickable(true);
		//column.set_sort_column_id(LogSourcesColumns::ChildCount as i32);

		{
			let renderer_text = CellRendererText::new();
			gtk::prelude::CellRendererExt::set_alignment(&renderer_text, 0.0, 0.0);
			column.pack_start(&renderer_text, false);
			column.add_attribute(&renderer_text, "text", LogSourcesColumns::ChildCount as i32);
		}
		sources_tree_view.append_column(&column);
	}

	fn build_left_store(
		store: &TreeStore,
		log_source: &LogSourceExt,
		parent: Option<&gtk::TreeIter>,
	) {
		let new_parent = store.insert_with_values(
			parent,
			None,
			&[
				(LogSourcesColumns::Active as u32, &true),
				(LogSourcesColumns::Inconsistent as u32, &false),
				(LogSourcesColumns::Text as u32, &log_source.name),
				(LogSourcesColumns::Id as u32, &log_source.id),
				(LogSourcesColumns::ChildCount as u32, &log_source.child_cnt),
			],
		);
		match &log_source.children {
			LogSourceContentsExt::Sources(v) => {
				for source in v {
					build_left_store(store, source, Some(&new_parent));
				}
			}
			LogSourceContentsExt::Entries(_v) => (),
		}
	}
	build_left_store(&left_store, &log_source_root_ext, None);
	sources_tree_view.expand_row(&gtk::TreePath::new_first(), false);
	//sources_tree_view.expand_all();

	let split_pane = gtk::Paned::new(Orientation::Horizontal);

	let scrolled_window_left =
		gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
	scrolled_window_left.set_property("overlay-scrolling", &false);
	scrolled_window_left.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
	//scrolled_window_left.set_property("min-content-width", &200);
	scrolled_window_left.add(&sources_tree_view);

	let split_pane_left = gtk::Box::new(Orientation::Vertical, 10);

	fn severity_toggle(
		w: &gtk::CheckButton,
		store: &mut LogStoreLinear,
		severity: model::LogLevel,
		drawing_area: &gtk::DrawingArea,
	) {
		log::info!("Active: {} ({:?})", w.is_active(), severity);
		store.filter_store(
			&|entry: &LogEntryExt| entry.severity == severity,
			w.is_active(),
			crate::model_internal::VISIBLE_OFF_SEVERITY,
		);
		drawing_area.queue_draw();
	}

	split_pane_left.pack_start(&scrolled_window_left, true, true, 0);
	{
		let check_btn = gtk::CheckButton::with_label("Critical");
		check_btn.set_active(true);

		let store_rc_clone = store_rc.clone();
		let drawing_area_clone = drawing_area.clone();
		check_btn.connect_clicked(move |w| {
			severity_toggle(
				w,
				&mut store_rc_clone.borrow_mut(),
				model::LogLevel::Critical,
				&drawing_area_clone,
			);
		});

		split_pane_left.pack_start(&check_btn, false, false, 0);
		let check_btn = gtk::CheckButton::with_label("Error");
		check_btn.set_active(true);

		let store_rc_clone = store_rc.clone();
		let drawing_area_clone = drawing_area.clone();
		check_btn.connect_clicked(move |w| {
			severity_toggle(
				w,
				&mut store_rc_clone.borrow_mut(),
				model::LogLevel::Error,
				&drawing_area_clone,
			);
		});

		split_pane_left.pack_start(&check_btn, false, false, 0);
		let check_btn = gtk::CheckButton::with_label("Warning");
		check_btn.set_active(true);

		let store_rc_clone = store_rc.clone();
		let drawing_area_clone = drawing_area.clone();
		check_btn.connect_clicked(move |w| {
			severity_toggle(
				w,
				&mut store_rc_clone.borrow_mut(),
				model::LogLevel::Warning,
				&drawing_area_clone,
			);
		});

		split_pane_left.pack_start(&check_btn, false, false, 0);
		let check_btn = gtk::CheckButton::with_label("Info");
		check_btn.set_active(true);

		let store_rc_clone = store_rc.clone();
		let drawing_area_clone = drawing_area.clone();
		check_btn.connect_clicked(move |w| {
			severity_toggle(
				w,
				&mut store_rc_clone.borrow_mut(),
				model::LogLevel::Info,
				&drawing_area_clone,
			);
		});

		split_pane_left.pack_start(&check_btn, false, false, 0);
		let check_btn = gtk::CheckButton::with_label("Debug");
		check_btn.set_active(true);

		let store_rc_clone = store_rc.clone();
		let drawing_area_clone = drawing_area.clone();
		check_btn.connect_clicked(move |w| {
			severity_toggle(
				w,
				&mut store_rc_clone.borrow_mut(),
				model::LogLevel::Debug,
				&drawing_area_clone,
			);
		});

		split_pane_left.pack_start(&check_btn, false, false, 0);
		let check_btn = gtk::CheckButton::with_label("Trace");
		check_btn.set_active(true);

		let store_rc_clone = store_rc.clone();
		let drawing_area_clone = drawing_area.clone();
		check_btn.connect_clicked(move |w| {
			severity_toggle(
				w,
				&mut store_rc_clone.borrow_mut(),
				model::LogLevel::Trace,
				&drawing_area_clone,
			);
		});

		split_pane_left.pack_start(&check_btn, false, false, 0);
	}

	fn search_changed(
		w: &gtk::SearchEntry,
		store: &mut LogStoreLinear,
		drawing_area: &gtk::DrawingArea,
	) {
		let search_text = w.text().as_str().to_string();
		if search_text.is_empty() {
			log::info!("Search empty");
			store.filter_store(
				&|_entry: &LogEntryExt| true,
				true,
				crate::model_internal::VISIBLE_OFF_FILTER,
			);
		} else {
			log::info!("search_changed {}", &search_text);
			store.filter_store(
				&|entry: &LogEntryExt| entry.message.contains(&search_text),
				true,
				crate::model_internal::VISIBLE_OFF_FILTER,
			);
			store.filter_store(
				&|entry: &LogEntryExt| !entry.message.contains(&search_text),
				false,
				crate::model_internal::VISIBLE_OFF_FILTER,
			);
		}
		drawing_area.queue_draw();
	}

	let search_entry = gtk::SearchEntry::new();
	let store_rc_clone = store_rc.clone();
	let drawing_area_clone = drawing_area.clone();
	search_entry.connect_search_changed(move |w| {
		search_changed(w, &mut store_rc_clone.borrow_mut(), &drawing_area_clone);
	});

	split_pane_left.pack_start(&search_entry, false, false, 0);

	let timediff_entry = gtk::Entry::new();
	timediff_entry.set_editable(false);
	timediff_entry.set_alignment(1.0); //1.0 is right-aligned
	timediff_entry.set_text("+0D 00:00:00.000");
	let timediff_label = gtk::Label::new(Some("Δt (hover-anchor):"));
	let timediff_box = gtk::Box::new(Orientation::Horizontal, 4);
	timediff_box.pack_start(&timediff_label, false, false, 0);
	timediff_box.pack_start(&timediff_entry, true, true, 0);
	split_pane_left.pack_start(&timediff_box, false, false, 0);

	split_pane.pack1(&split_pane_left, false, false);

	//https://developer.gnome.org/gtk3/stable/GtkPaned.html

	// Assemble log store ----------------------------------------------------------

	fn build_log_store(store: &mut Vec<LogEntryExt>, log_source: &mut LogSourceExt) {
		match &mut log_source.children {
			LogSourceContentsExt::Sources(v) => {
				for source in v {
					build_log_store(store, source);
				}
			}
			LogSourceContentsExt::Entries(v) => {
				store.append(v);
			}
		}
	}

	log::info!("before build_log_store");
	let now = Instant::now();
	build_log_store(&mut store_rc.borrow_mut().store, &mut log_source_root_ext);

	fn build_log_sources(
		log_sources: &mut std::collections::HashMap<u32, String>,
		log_source: &LogSourceExt,
		prefix: String,
	) {
		let current_name = String::new() + &prefix + "/" + &log_source.name;
		log_sources.insert(log_source.id, current_name.clone());
		match &log_source.children {
			LogSourceContentsExt::Sources(v) => {
				for source in v {
					build_log_sources(log_sources, source, current_name.clone());
				}
			}
			LogSourceContentsExt::Entries(_) => (),
		}
	}

	build_log_sources(
		&mut store_rc.borrow_mut().log_sources,
		&log_source_root_ext,
		String::new(),
	);

	store_rc
		.borrow_mut()
		.store
		.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
	store_rc.borrow_mut().filter_store(
		&|_entry: &LogEntryExt| true,
		true,
		crate::model_internal::VISIBLE_OFF_SOURCE,
	); //set all to active, initialize ids

	let elapsed = now.elapsed();
	log::info!(
		"Time to create store: {}ms",
		elapsed.as_secs() * 1000 + elapsed.subsec_millis() as u64
	);
	log::info!("after build_log_store");

	//-------------------------------------------------------------------------------

	let event_mask = EventMask::POINTER_MOTION_MASK
		| EventMask::BUTTON_PRESS_MASK
		| EventMask::BUTTON_RELEASE_MASK
		| EventMask::KEY_PRESS_MASK
		| EventMask::KEY_RELEASE_MASK
		| EventMask::SCROLL_MASK;

	drawing_area.set_can_focus(true);
	drawing_area.add_events(event_mask);
	let f_clone_1 = store_rc.clone();
	drawing_area.connect_event(move |x, y| handle_evt(&mut f_clone_1.borrow_mut(), x, y));

	// establish a reasonable minimum view size
	drawing_area.set_size_request(200, 200);

	// https://gtk-rs.org/docs/gtk/trait.WidgetExt.html
	let f_clone_2 = store_rc.clone();
	drawing_area.connect_draw(move |x, y| draw(&mut f_clone_2.borrow_mut(), x, y));

	let f_clone_3 = store_rc.clone();
	drawing_area
		.connect_scroll_event(move |x, y| handle_evt_scroll(&mut f_clone_3.borrow_mut(), x, y));

	let f_clone_4 = store_rc.clone();
	drawing_area.connect_button_press_event(move |x, y| {
		handle_evt_press(&mut f_clone_4.borrow_mut(), x, y)
	});

	let f_clone_5 = store_rc.clone();
	drawing_area.connect_button_release_event(move |x, y| {
		handle_evt_release(&mut f_clone_5.borrow_mut(), x, y)
	});

	let f_clone_6 = store_rc.clone();
	drawing_area.connect_motion_notify_event(move |x, y| {
		handle_evt_motion(&mut f_clone_6.borrow_mut(), &timediff_entry.clone(), x, y)
	});

	split_pane.pack2(&drawing_area, true, false);

	//https://gtk-rs.org/docs/gdk/enums/key/index.html
	//log::info!("CODES: {} {} {} {}", gdk::keys::constants::Control_L, gdk::keys::constants::Control_R, gdk::keys::constants::Shift_L, gdk::keys::constants::Shift_R);
	/*You should place GtkDrawArea in GtkEventBox and then doing all that stuff from GtkEventBox. As far as I remember, this is happening because there are not these events for GtkDrawArea. One in stackoverflow explained that, but only with GtkImage. I know, that GtkDrawArea in GtkEventBox works, because I am currently writing app that uses it (app is in c, but it should work for c++ too).
	https://stackoverflow.com/questions/52171141/gtkmm-how-to-attach-keyboard-events-to-an-drawingarea*/
	{
		let store_rc_clone = store_rc.clone();
		window.connect_key_press_event(move |_window, event_key| {
			log::info!(
				"KEY PRESSED! {} {}",
				event_key.keyval(),
				event_key.hardware_keycode()
			);
			if event_key.keyval() == gdk::keys::constants::Control_L
				|| event_key.keyval() == gdk::keys::constants::Control_R
			{
				store_rc_clone.borrow_mut().pressed_ctrl = true;
			}
			if event_key.keyval() == gdk::keys::constants::Shift_L
				|| event_key.keyval() == gdk::keys::constants::Shift_R
			{
				store_rc_clone.borrow_mut().pressed_shift = true;
			}
			if event_key.keyval() == gdk::keys::constants::c && store_rc_clone.borrow().pressed_ctrl
			{
				let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);

				//TODO: 13.04.2020: Can optimize this, use some sort of string stream
				let mut clip_string = std::string::String::new();

				//TODO: 13.04.2020: Can optimize this, do not go through entire store
				for (offset, entry) in store_rc_clone
					.borrow()
					.store
					.iter()
					.enumerate() //offset in vector
					.filter(|(_, x)| x.is_visible())
				{
					//TODO: 13.04.2020: Clean up all these borrows.
					if (store_rc_clone.borrow().selected_single.contains(&offset)
						|| (store_rc_clone.borrow().selected_range.is_some()
							&& store_rc_clone.borrow().selected_range.unwrap().0 <= offset
							&& store_rc_clone.borrow().selected_range.unwrap().1 >= offset))
						&& !store_rc_clone.borrow().excluded_single.contains(&offset)
					{
						//TODO: 13.04.2020: Also add log source name to string!
						clip_string += &entry.timestamp.format("%d-%m-%y %T%.6f").to_string();
						clip_string += &" | ";
						clip_string += &entry.message;
						clip_string += &"\r\n"; //TODO: 13.04.2020: Windows vs Linux file endings?
					}
				}
				clipboard.set_text(&clip_string);
			}
			gtk::Inhibit(false)
		});
	}
	{
		#[allow(clippy::redundant_clone)]
		let store_rc_clone = store_rc.clone();
		window.connect_key_release_event(move |_window, event_key| {
			log::info!(
				"KEY RELEASED! {} {}",
				event_key.keyval(),
				event_key.hardware_keycode()
			);
			if event_key.keyval() == gdk::keys::constants::Control_L
				|| event_key.keyval() == gdk::keys::constants::Control_R
			{
				store_rc_clone.borrow_mut().pressed_ctrl = false;
			}
			if event_key.keyval() == gdk::keys::constants::Shift_L
				|| event_key.keyval() == gdk::keys::constants::Shift_R
			{
				store_rc_clone.borrow_mut().pressed_shift = false;
			}
			gtk::Inhibit(false)
		});
	}

	window.add(&split_pane);
	window.show_all();

	for dialog in dialog_vec {
		dialog.run();
		dialog.emit_close();
	}
}

fn gio_files_to_paths(gio_files: &[gio::File]) -> Vec<std::path::PathBuf> {
	let mut result = Vec::new();
	for gio_file in gio_files {
		result.push(gio_file.path().expect("Invalid file path"));
	}
	result
}

fn main() {
	fern::Dispatch::new()
		// Perform allocation-free log formatting
		.format(|out, message, record| {
			out.finish(format_args!(
				"{}[{}][{}] {}",
				chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
				record.target(),
				record.level(),
				message
			))
		})
		.level(log::LevelFilter::Warn)
		.level_for("sherlog", log::LevelFilter::Trace)
		.chain(std::io::stdout())
		//.chain(fern::log_file("output.log").unwrap())
		// Apply globally
		.apply()
		.unwrap();

	// https://developer.gnome.org/CommandLine/
	// https://developer.gnome.org/GtkApplication/

	let application = gtk::Application::new(
		Some("com.github.BenjaminRi.Sherlog"),
		gio::ApplicationFlags::HANDLES_OPEN,
	);

	// ---------------------------------------------------------------
	// Log handling: Note that glib_sys::g_log_set_writer_func is not
	// yet exposed via glib, therefore, we cannot do structured logging
	// and we cannot catch Gtk errors! Structured logging completely
	// ignores unstructured log handlers (glib::log_set_handler).

	//fn printerr(msg: &str) {
	//	log::info!("RustA: {}", msg);
	//}

	//fn printerr2(a: Option<&str>, b: glib::LogLevel, c: &str) {
	//	log::info!("RustB: {:?}, {:?}, {}", a, b, c);
	//}

	//https://developer.gnome.org/glib/stable/glib-Warnings-and-Assertions.html
	//glib::set_printerr_handler(printerr);
	//glib::set_print_handler(printerr);

	//https://stackoverflow.com/questions/39509231/gdb-debugging-with-breakpoint-gtk-warning-invalid-text-buffer-iterator
	//https://developer.gnome.org/glib/stable/glib-Message-Logging.html#g-log-set-handler
	//glib::log_set_handler(Some("Gtk"), glib::LogLevels::all(), true, true, printerr2);
	//glib::log_set_default_handler(printerr2);

	//glib::g_log!("test", LogLevel::Warning, "test");
	//glib::g_warning!("test2", "test2");
	//glib::g_log!("Gtk", LogLevel::Warning, "test");
	//glib::g_warning!("Gtk", "test2");

	// End of log handling. Have to wait until Gtk fixes this problem.
	// ---------------------------------------------------------------

	// https://gtk-rs.org/docs/glib/struct.OptionFlags.html
	// https://gtk-rs.org/docs/glib/enum.OptionArg.html
	application.add_main_option(
		"CTest",
		glib::Char::from(b'c'),
		glib::OptionFlags::IN_MAIN,
		glib::OptionArg::String,
		"This is just a test",
		Some("This is a test argument"),
	);

	// https://gtk-rs.org/docs/gio/prelude/trait.ApplicationExtManual.html
	application.connect_open(move |app, gio_files, _| {
		build_ui(app, &gio_files_to_paths(gio_files));
	});

	application.connect_activate(|app| {
		build_ui(app, &Vec::new());
	});

	// https://gtk-rs.org/docs/gio/prelude/trait.ApplicationExtManual.html#tymethod.run
	application.run();
}
*/

/*
DateTime utilities:
let dt = Utc.ymd(2018, 1, 26).and_hms_micro(18, 30, 9, 453_829);
log::info!("{}", dt.to_rfc3339_opts(SecondsFormat::Millis, false));
let ts_milli : u64 = 1568208334469;
let ts_sec   : u64 = ts_milli / 1000;
let ts_nano  : u32 = ((ts_milli - ts_sec * 1000) * 1000_000) as u32;
let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp_opt(ts_sec as i64, ts_nano).expect("Invalid timestamp encountered"), Utc);
log::info!("{}", dt.to_rfc3339_opts(SecondsFormat::Millis, false));
let dt : DateTime::<Utc> = DateTime::<FixedOffset>::parse_from_rfc3339("1996-12-19T16:39:57-08:00").expect("Parse error!").with_timezone(&Utc);
log::info!("{}", dt.to_rfc3339_opts(SecondsFormat::Millis, false));
*/

/*
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
