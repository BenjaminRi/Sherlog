extern crate gio;
extern crate gtk;
extern crate gdk;
extern crate glib;
extern crate log;
extern crate chrono;
extern crate cairo;

use gio::prelude::*;
use gtk::prelude::*;
use gtk::DrawingArea;
use gdk::EventMask;

use std::rc::Rc;
use std::cell::RefCell;

use std::time::SystemTime;

#[allow(unused_imports)]
use regex::Regex;

mod parse;
mod log_store;
mod model;
mod model_internal;
mod tree_model;

use log_store::LogStoreLinear;
use log_store::ScrollBarVert;

use model_internal::LogEntryExt;
use model_internal::LogSourceContentsExt;
use model_internal::LogSourceExt;

#[allow(unused_imports)]
use gtk::{
    ApplicationWindow, ButtonsType, CellRendererPixbuf, CellRendererText, DialogFlags,
    MessageDialog, MessageType, Orientation, TreeStore, ListStore, TreeView, TreeViewColumn, WindowPosition,
};

use std::env::args;

enum LogSourcesColumns {
    Active = 0,
	Inconsistent = 1,
	Text = 2,
	Id = 3,
	ChildCount = 4,
}

fn fixed_toggled_sorted<W: IsA<gtk::CellRendererToggle>>(
	tree_store: &gtk::TreeStore,
    model_sort: &gtk::TreeModelSort,
	store: &mut LogStoreLinear,
	drawing_area : &gtk::DrawingArea,
    _w: &W,
    path: gtk::TreePath,
) {
	fixed_toggled(
		tree_store,
		store,
		drawing_area,
		_w,
		model_sort.convert_path_to_child_path(&path).unwrap());
}

fn fixed_toggled<W: IsA<gtk::CellRendererToggle>>(
    tree_store: &gtk::TreeStore,
	store: &mut LogStoreLinear,
	drawing_area : &gtk::DrawingArea,
    _w: &W,
    path: gtk::TreePath,
) {
	//println!("Path: {:?}", path.get_indices_with_depth());
	
    let iter = tree_store.get_iter(&path).unwrap();
    let mut active = tree_store
        .get_value(&iter, LogSourcesColumns::Active as i32)
        .get_some::<bool>()
        .unwrap();
	let mut inconsistent = tree_store
        .get_value(&iter, LogSourcesColumns::Inconsistent as i32)
        .get_some::<bool>()
        .unwrap();
	
	
	if inconsistent || !active {
		inconsistent = false;
		active = true;
	} else {
		active = false;
	}
	
    tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &active.to_value());
	tree_store.set_value(&iter, LogSourcesColumns::Inconsistent as u32, &inconsistent.to_value());
	
	fn check_inconsistent(tree_store: &gtk::TreeStore, mut path: gtk::TreePath) -> bool{
		let mut prev_active = None;
		if path.up() {
			path.append_index(0);
			while let Some(iter) = tree_store.get_iter(&path) {
				let n_active = tree_store
					.get_value(&iter, LogSourcesColumns::Active as i32)
					.get_some::<bool>()
					.unwrap();
				
				let n_inconsistent = tree_store
					.get_value(&iter, LogSourcesColumns::Inconsistent as i32)
					.get_some::<bool>()
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
		let mut path_up = path.clone();
		let mut level_inconsistent = check_inconsistent(tree_store, path_up.clone());
		while path_up.up() && path_up.get_depth() > 0 {
			let iter = tree_store.get_iter(&path_up).unwrap();
			if level_inconsistent {
				tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &false.to_value());
			}
			else
			{
				tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &active.to_value());
			}
			tree_store.set_value(&iter, LogSourcesColumns::Inconsistent as u32, &level_inconsistent.to_value());
			level_inconsistent = check_inconsistent(tree_store, path_up.clone());
		}
	}
	
	fn activate_children(tree_store: &gtk::TreeStore, mut path: gtk::TreePath, active : bool, sources: &mut Vec::<u32>) {
		//println!("activate_children... {:?}", path.get_indices_with_depth());
		path.down();
		while let Some(iter) = tree_store.get_iter(&path)
		{
			let n_active = tree_store
					.get_value(&iter, LogSourcesColumns::Active as i32)
					.get_some::<bool>()
					.unwrap();
			if n_active != active {
				tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &active.to_value());
			}
			let n_id = tree_store
				.get_value(&iter, LogSourcesColumns::Id as i32)
				.get_some::<u32>()
				.unwrap();
			//println!("activate_children... {}", n_id);
			sources.push(n_id); //Don't just push diffs. Push continuous ranges to enable optimization below.
			tree_store.set_value(&iter, LogSourcesColumns::Inconsistent as u32, &false.to_value());
			activate_children(tree_store, path.clone(), active, sources);
			path.next();
		}
	}
	let mut sources = Vec::<u32>::new();
	let id = tree_store
		.get_value(&iter, LogSourcesColumns::Id as i32)
		.get_some::<u32>()
		.unwrap();
	sources.push(id);
	activate_children(tree_store, path, active, &mut sources);
	//println!("Click: {:?} change to {}", sources, active); //Note: Very verbose output.
	
	let mut ordered = true;
	let mut next_id = *sources.first().unwrap(); //We can do this because we know we pushed at least one id above.
	for id in sources.iter() {
		if next_id != *id {
			ordered = false;
			break;
		}
		else
		{
			next_id += 1;
		}
	}
	
	if !ordered {
		println!("ERROR: Unordered log source tree detected!");
		panic!(); //If this happens you broke the tree structure
	}
	
	let first_id = *sources.first().unwrap(); //We can do this because we know we pushed at least one id above.
	let last_id = *sources.last().unwrap(); //We can do this because we know we pushed at least one id above.
	
	//println!("Click: Range [{},{}] set to {}", first_id, last_id, active);
	//println!("Inconsistent: {}", level_inconsistent);
	
	/*
	let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
	assert!(re.is_match("2014-01-01"));
	let re = Regex::new(r"job").unwrap();
	re.is_match(&entry.message)
	*/
	
	let now = SystemTime::now();
	store.filter_store(&|entry: &LogEntryExt| { entry.source_id >= first_id && entry.source_id <= last_id }, active, crate::model_internal::VISIBLE_OFF_SOURCE);
	match now.elapsed() {
		Ok(elapsed) => {
			println!("Time to update store: {}ms", elapsed.as_secs()*1000+elapsed.subsec_millis() as u64);
		}
		Err(e) => {
			// an error occurred!
			println!("Error: {:?}", e);
		}
	}
	
	drawing_area.queue_draw();
}


//--------------------------------------------------------------------------------------------------
//--------------------------------------------------------------------------------------------------
//--------------------------------------------------------------------------------------------------
//--------------------------------------------------------------------------------------------------

fn draw(store: &mut LogStoreLinear, drawing_area: &DrawingArea, ctx: &cairo::Context) -> gtk::Inhibit {
		//store.store.push(model::LogEntry { message: "TestTrace 309468456".to_string(),       severity: model::LogLevel::Trace,    ..Default::default() });
		//println!("{}", store.store.len());
		
		//println!("w: {} h: {}", drawing_area.get_allocated_width(), drawing_area.get_allocated_height());
		
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
		
		let h = drawing_area.get_allocated_height();
		let w = drawing_area.get_allocated_width();
		
		ctx.set_source_rgb(0.0, 0.0, 0.0);
		
		store.visible_lines = (h/25) as usize;
		
		if store.store.len() < store.visible_lines {
			//No scrolling possible, less entries than rows on GUI!
			store.cursor_pos = 0;
		}else if store.cursor_pos > store.store.len() - store.visible_lines {
			store.cursor_pos = store.store.len() - store.visible_lines; 
		}
		
		let mut i = 0;
		for entry in store.store.iter().skip(store.cursor_pos).filter(|x| x.is_visible()).take(store.visible_lines) {
			i += 1;
			
			ctx.select_font_face("Lucida Console", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
			ctx.set_font_size(14.0);
			
			match entry.severity {
				model::LogLevel::Critical => { ctx.set_source_rgb(0.5, 0.0, 0.0); }, // Dark red
				model::LogLevel::Error => { ctx.set_source_rgb(1.0, 0.0, 0.0); }, //Red
				model::LogLevel::Warning => { ctx.set_source_rgb(0.77, 0.58, 0.0); }, //Dirty yellow-orange
				model::LogLevel::Info => { ctx.set_source_rgb(0.0, 0.0, 0.0); }, //Black
				model::LogLevel::Debug => { ctx.set_source_rgb(0.6, 0.6, 0.6); }, //Grey
				model::LogLevel::Trace => { ctx.set_source_rgb(0.4, 0.4, 0.4); }, //Light grey
			}
			
			if !entry.is_visible() {
				ctx.set_source_rgb(0.9, 0.9, 0.9);
			}
			
			let date_str = entry.timestamp.format("%y-%m-%d %T%.3f").to_string();
			ctx.move_to(30.0, 20.0+20.0* i as f64);
			ctx.show_text(&date_str);
			
			let short_sev = match entry.severity {
				model::LogLevel::Critical => "CRI",
				model::LogLevel::Error => "ERR",
				model::LogLevel::Warning => "WRN",
				model::LogLevel::Info => "INF",
				model::LogLevel::Debug => "DBG",
				model::LogLevel::Trace => "TRC",
			};
			
			ctx.move_to(210.0, 20.0+20.0* i as f64);
			ctx.show_text(&short_sev);
			
			
			ctx.move_to(240.0, 20.0+20.0* i as f64);
			
			/*let font_face = ctx.get_font_face();
			let new_font_face = cairo::FontFace::toy_create("cairo :monospace", font_face.toy_get_slant(), font_face.toy_get_weight());
			ctx.set_font_face(&new_font_face);*/
			
			ctx.show_text(&entry.message);
		}
		
		{
			store.scroll_bar.bar_height = h as f64 - store.scroll_bar.bar_padding * 2.0;
			
			
			store.scroll_bar.scroll_perc = store.get_scroll_percentage(store.visible_lines);
			
			store.scroll_bar.thumb_rel_offset = f64::round((store.scroll_bar.bar_height - store.scroll_bar.thumb_height - store.scroll_bar.thumb_margin * 2.0) * store.scroll_bar.scroll_perc) + store.scroll_bar.thumb_margin;
			
			store.scroll_bar.x = w as f64 - store.scroll_bar.bar_width - store.scroll_bar.bar_padding;
			store.scroll_bar.y = store.scroll_bar.bar_padding;
			
			store.scroll_bar.thumb_width = store.scroll_bar.bar_width - 2.0 * store.scroll_bar.thumb_margin;
			store.scroll_bar.thumb_x = store.scroll_bar.x + store.scroll_bar.thumb_margin;
			store.scroll_bar.thumb_y = store.scroll_bar.y + store.scroll_bar.thumb_rel_offset;
			
			ctx.set_source_rgb(0.7, 0.7, 0.7);
			ctx.rectangle(store.scroll_bar.x, store.scroll_bar.y, store.scroll_bar.bar_width, store.scroll_bar.bar_height);
			ctx.fill();
			
			ctx.set_source_rgb(0.3, 0.3, 0.3);
			ctx.rectangle(store.scroll_bar.thumb_x, store.scroll_bar.thumb_y, store.scroll_bar.thumb_width, store.scroll_bar.thumb_height);
			ctx.fill();
			
		}
		
		
		gtk::Inhibit(false)
	}
	
	fn handle_evt(_store: &mut LogStoreLinear, _drawing_area: &DrawingArea, evt: &gdk::Event) -> gtk::Inhibit {
		//_drawing_area.queue_draw();
		if evt.get_event_type() != gdk::EventType::MotionNotify {
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
			println!("Event: {:?}", evt.get_event_type());
		}
		gtk::Inhibit(false)
	}
	
	fn handle_evt_scroll(store: &mut LogStoreLinear, drawing_area: &DrawingArea, evt: &gdk::EventScroll) -> gtk::Inhibit {
		let scroll_speed = 3;
		let mut dirty = false;
		match evt.get_direction() {
			gdk::ScrollDirection::Up => {
					dirty = store.scroll(-scroll_speed, store.visible_lines);
				},
			gdk::ScrollDirection::Down => {
					dirty = store.scroll(scroll_speed, store.visible_lines);
				},
			_ => ()
		}
		
		if dirty {
			drawing_area.queue_draw();
		}
		
		println!("Scroll... dirty: {}, pos: {}", dirty, store.cursor_pos);
		
		gtk::Inhibit(false)
	}
	
	fn handle_evt_press(store: &mut LogStoreLinear, _drawing_area: &DrawingArea, evt: &gdk::EventButton) -> gtk::Inhibit {
		/*let h = _drawing_area.get_allocated_height();
		let mut scroll_perc = evt.get_position().1/h as f64;
		if scroll_perc < 0.0 {
			scroll_perc = 0.0;
		} else if scroll_perc > 1.0 {
			scroll_perc = 1.0;
		}
		store.cursor_pos = f64::round(scroll_perc * (store.store.len() - 1) as f64) as usize;
		_drawing_area.queue_draw();*/
		//println!("PRESS pos:  {:?}", evt.get_position());
		//println!("PRESS root: {:?}", evt.get_root());
		
		store.mouse_down = true;
		if
		evt.get_position().0 >= store.scroll_bar.thumb_x &&
		evt.get_position().0 <= store.scroll_bar.thumb_x + store.scroll_bar.thumb_width &&
		evt.get_position().1 >= store.scroll_bar.thumb_y &&
		evt.get_position().1 <= store.scroll_bar.thumb_y + store.scroll_bar.thumb_height {
			store.thumb_drag = true;
			store.thumb_drag_x = evt.get_position().0 - store.scroll_bar.thumb_x;
			store.thumb_drag_y = evt.get_position().1 - store.scroll_bar.thumb_y;
		}
		gtk::Inhibit(false)
	}
	
	fn handle_evt_release(store: &mut LogStoreLinear, _drawing_area: &DrawingArea, _evt: &gdk::EventButton) -> gtk::Inhibit {
		println!("RELEASE");
		store.mouse_down = false;
		store.thumb_drag = false;
		store.thumb_drag_x = 0.0;
		store.thumb_drag_y = 0.0;
		gtk::Inhibit(false)
	}
	
	fn handle_evt_motion(store: &mut LogStoreLinear, drawing_area: &DrawingArea, evt: &gdk::EventMotion) -> gtk::Inhibit {
		if store.thumb_drag {
			store.scroll_bar.thumb_y = evt.get_position().1 - store.thumb_drag_y;
			store.scroll_bar.thumb_rel_offset = store.scroll_bar.thumb_y - store.scroll_bar.y;
			store.scroll_bar.scroll_perc = (store.scroll_bar.thumb_rel_offset - store.scroll_bar.thumb_margin) / (store.scroll_bar.bar_height - store.scroll_bar.thumb_height - store.scroll_bar.thumb_margin * 2.0);
			if store.scroll_bar.scroll_perc < 0.0 {
				store.scroll_bar.scroll_perc = 0.0;
			} else if store.scroll_bar.scroll_perc > 1.0 {
				store.scroll_bar.scroll_perc = 1.0;
			}
			store.cursor_pos = store.percentage_to_offset(store.scroll_bar.scroll_perc, store.visible_lines).unwrap_or(0);
			println!("MOTION {:?}", evt.get_position());
			drawing_area.queue_draw();
		}
		gtk::Inhibit(false)
	}

//--------------------------------------------------------------------------------------------------
//--------------------------------------------------------------------------------------------------
//--------------------------------------------------------------------------------------------------

fn build_ui(application: &gtk::Application, file_paths: &[std::path::PathBuf]) {
    let window = gtk::ApplicationWindow::new(application);
	//window.set_icon_from_file("../images/sherlog_icon.png");
    window.set_title("Sherlog");
    window.set_border_width(10);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(600, 400);
	
	//let args = &args().collect::<Vec<_>>();
    /*//FIXME: Use handle-local-options once https://github.com/gtk-rs/gtk/issues/580 is a thing
    let mut new_instance = false;
    for arg in args {
        match arg.as_str() {
            "-n" | "--new-instance" => new_instance = true,
            _ => (),
        }
    }*/
    //println!("{:?}", args);
	
	//Generate fake log entries to test GUI ---------------------------------------------
	
	let log_entries = vec![
		model::LogEntry { message: "TestCritical 121343245345".to_string(), severity: model::LogLevel::Critical, ..Default::default() },
		model::LogEntry { message: "TestError 3405834068".to_string(),      severity: model::LogLevel::Error,    ..Default::default() },
		model::LogEntry { message: "TestWarning 340958349068".to_string(),  severity: model::LogLevel::Warning,  ..Default::default() },
		model::LogEntry { message: "TestInfo 3049580349568".to_string(),    severity: model::LogLevel::Info,     ..Default::default() },
		model::LogEntry { message: "TestDebug 0345986045968".to_string(),   severity: model::LogLevel::Debug,    ..Default::default() },
		model::LogEntry { message: "TestTrace 309468456".to_string(),       severity: model::LogLevel::Trace,    ..Default::default() },
		];
	
	//Vec::<model::LogEntry>::new()
	let log_source_ex = model::LogSource {name: "example".to_string(), children: {model::LogSourceContents::Entries(log_entries) } };
	let log_source_ex2_1 = model::LogSource {name: "example2_1".to_string(), children: {model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) } };
	let log_source_ex2 = model::LogSource {name: "example2".to_string(), children: {model::LogSourceContents::Sources(vec![log_source_ex2_1]) } };
	let log_source_ex3 = model::LogSource {name: "example3".to_string(), children: {model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) } };
	let log_source_ex4_1 = model::LogSource {name: "example4_1".to_string(), children: {model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) } };
	let log_source_ex4_2 = model::LogSource {name: "example4_2".to_string(), children: {model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) } };
	let log_source_ex4 = model::LogSource {name: "examale4".to_string(), children: {model::LogSourceContents::Sources(vec![log_source_ex4_1, log_source_ex4_2]) } };
	
	let log_source_root = model::LogSource {name: "Root LogSource".to_string(), children: {model::LogSourceContents::Sources(
	vec![log_source_ex, log_source_ex2, log_source_ex3, log_source_ex4]) } };
	
	//---------------------------------------------------------------------------------------
	
	println!("{:?}", file_paths);
	let log_source_root = if !file_paths.is_empty() {
		if file_paths.len() > 1 {
			println!("WARNING: Multiple files opened, ignoring all but the first one.");
		}
		parse::from_file(&file_paths[0]).expect("Could not read file!")
	} else {
		log_source_root
	};
	
	// Create log store as Refcounted RefCell to be used in closures ------------------------
		
	let store = LogStoreLinear {
		store : Vec::<LogEntryExt>::new(),
		entry_count : 0,
		first_offset : 0,
		last_offset : 0,
	
		cursor_pos : 0,
		visible_lines : 0,
		mouse_down : false,
		thumb_drag : false,
		thumb_drag_x : 0.0,
		thumb_drag_y : 0.0,
		scroll_bar : ScrollBarVert {
			x : 0.0,
			y : 0.0,
			
			bar_padding : 10.0,
			bar_width : 20.0,
			bar_height : 0.0, //calculate dynamically
			
			thumb_x : 0.0, //calculate dynamically
			thumb_y : 0.0, //calculate dynamically
			thumb_margin : 3.0,
			thumb_width : 0.0, //calculate dynamically
			thumb_height : 50.0,
			thumb_rel_offset : 0.0, //calculate dynamically
			
			scroll_perc : 0.0, //calculate dynamically
		}
	};
	
	let store_rc = Rc::new(RefCell::new(store));
	
	//---------------------------------------------------------------------------------------
	
	let drawing_area = DrawingArea::new();
	
	let mut log_source_root_ext = LogSourceExt::from_source(log_source_root);
	
	// left pane
    let left_store = TreeStore::new(&[glib::Type::Bool, glib::Type::Bool, String::static_type(), glib::Type::U32, glib::Type::U64]);
	let left_store_sort = gtk::TreeModelSort::new(&left_store);
	let sources_tree_view = gtk::TreeView::new_with_model(&left_store_sort);
    sources_tree_view.set_headers_visible(true);
	
	{
		//https://github.com/ChariotEngine/drs-studio/blob/f0303b52063f0d365732941e5096c42dad06f326/ui/gtk/src/main.rs
		let store_clone = left_store_sort.clone();
		left_store_sort.set_sort_func(gtk::SortColumn::Index(LogSourcesColumns::Text as u32), move |_w, l_it, r_it| {
			let l_id = store_clone
				.get_value(&l_it, LogSourcesColumns::ChildCount as i32)
				.get_some::<u64>()
				.unwrap();
			let r_id = store_clone
				.get_value(&r_it, LogSourcesColumns::ChildCount as i32)
				.get_some::<u64>()
				.unwrap();
			l_id.cmp(&r_id)
		} );
	}
	
	// Column with checkbox to toggle log sources, plus log source name
    {
		let column = gtk::TreeViewColumn::new();
		// https://lazka.github.io/pgi-docs/Gtk-3.0/classes/TreeViewColumn.html#Gtk.TreeViewColumn.set_sort_indicator
		column.set_sizing(gtk::TreeViewColumnSizing::Fixed);
		column.set_title("Log source");
		column.set_fixed_width(300);
		column.set_sort_indicator(true);
		column.set_clickable(true);
		column.set_sort_column_id(LogSourcesColumns::Text as i32);
		
		{
			let renderer_toggle = gtk::CellRendererToggle::new();
			//renderer_toggle.set_property_inconsistent(true);
			renderer_toggle.set_alignment(0.0, 0.0);
			//renderer_toggle.set_padding(0, 0);
			let left_store_clone = left_store.clone(); //GTK objects are refcounted, just clones ref
			let model_sort_clone = left_store_sort.clone(); //GTK objects are refcounted, just clones ref
			let drawing_area_clone = drawing_area.clone(); //GTK objects are refcounted, just clones ref
			let store_rc_clone = store_rc.clone();
			renderer_toggle.connect_toggled(move |w, path| fixed_toggled_sorted(&left_store_clone, &model_sort_clone, &mut store_rc_clone.clone().borrow_mut(), &drawing_area_clone, w, path));
			column.pack_start(&renderer_toggle, false);
			column.add_attribute(&renderer_toggle, "active", LogSourcesColumns::Active as i32);
			column.add_attribute(&renderer_toggle, "inconsistent", LogSourcesColumns::Inconsistent as i32);
		}
		
		{
			let renderer_text = CellRendererText::new();
			renderer_text.set_alignment(0.0, 0.0);
			column.pack_start(&renderer_text, false);
			column.add_attribute(&renderer_text, "text", LogSourcesColumns::Text as i32);
		}
		sources_tree_view.append_column(&column);
	}
	
	//Column with number of entries of a log source
	{
		let column = gtk::TreeViewColumn::new();
		column.set_title("Entries");
		column.set_sort_indicator(true);
		column.set_clickable(true);
		column.set_sort_column_id(LogSourcesColumns::ChildCount as i32);
		
		{
			let renderer_text = CellRendererText::new();
			renderer_text.set_alignment(0.0, 0.0);
			column.pack_start(&renderer_text, false);
			column.add_attribute(&renderer_text, "text", LogSourcesColumns::ChildCount as i32);
		}
		sources_tree_view.append_column(&column);
	}
	
	fn build_left_store(store: &TreeStore, log_source: &LogSourceExt, parent: Option<&gtk::TreeIter>) {
		let new_parent = store.insert_with_values(
			parent,
			None,
			&[
			LogSourcesColumns::Active as u32,
			LogSourcesColumns::Inconsistent as u32,
			LogSourcesColumns::Text as u32,
			LogSourcesColumns::Id as u32,
			LogSourcesColumns::ChildCount as u32,
			],
			&[
			&true,
			&false,
			&log_source.name,
			&log_source.id,
			&log_source.child_cnt
			]);
		match &log_source.children {
			LogSourceContentsExt::Sources(v) => {
				for source in v {
					build_left_store(store, source, Some(&new_parent));
				}
			},
			LogSourceContentsExt::Entries(_v) => {
				()
			},
		}
	}
	build_left_store(&left_store, &log_source_root_ext, None);
	sources_tree_view.expand_all();
	
	let split_pane = gtk::Box::new(Orientation::Horizontal, 10);

    split_pane.set_size_request(-1, -1);
	let scrolled_window_left = gtk::ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
	scrolled_window_left.set_property("overlay-scrolling", &false).unwrap();
	scrolled_window_left.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
	//scrolled_window_left.set_property("min-content-width", &200);
	scrolled_window_left.add(&sources_tree_view);
	
	let split_pane_left = gtk::Box::new(Orientation::Vertical, 10);
	split_pane_left.pack_start(&scrolled_window_left, true, true, 0);
	{
		let check_btn = gtk::CheckButton::new_with_label("Critical");
		check_btn.set_active(true);
		split_pane_left.pack_start(&check_btn, false, false, 0);
		let check_btn = gtk::CheckButton::new_with_label("Error");
		check_btn.set_active(true);
		split_pane_left.pack_start(&check_btn, false, false, 0);
		let check_btn = gtk::CheckButton::new_with_label("Warning");
		check_btn.set_active(true);
		split_pane_left.pack_start(&check_btn, false, false, 0);
		let check_btn = gtk::CheckButton::new_with_label("Info");
		check_btn.set_active(true);
		split_pane_left.pack_start(&check_btn, false, false, 0);
		let check_btn = gtk::CheckButton::new_with_label("Debug");
		check_btn.set_active(true);
		split_pane_left.pack_start(&check_btn, false, false, 0);
		let check_btn = gtk::CheckButton::new_with_label("Trace");
		check_btn.set_active(true);
		split_pane_left.pack_start(&check_btn, false, false, 0);
	}
	
	split_pane.pack_start(&split_pane_left, false, false, 0);
	
	
	//https://developer.gnome.org/gtk3/stable/GtkPaned.html
	
	// Assemble log store ----------------------------------------------------------
	
	fn build_log_store(store: &mut Vec<LogEntryExt>, log_source: &mut LogSourceExt) {
		match &mut log_source.children {
			LogSourceContentsExt::Sources(v) => {
				for source in v {
					build_log_store(store, source);
				}
			},
			LogSourceContentsExt::Entries(v) => {
				store.append(v);
				()
			},
		}
	}
	
	println!("before build_log_store");
	let now = SystemTime::now();
	build_log_store(&mut store_rc.borrow_mut().store, &mut log_source_root_ext);
	
	store_rc.borrow_mut().store.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
	store_rc.borrow_mut().filter_store(&|_entry: &LogEntryExt| { true }, true, crate::model_internal::VISIBLE_OFF_SOURCE); //set all to active, initialize ids
	
	match now.elapsed() {
		Ok(elapsed) => {
			println!("Time to create store: {}ms", elapsed.as_secs()*1000+elapsed.subsec_millis() as u64);
		}
		Err(e) => {
			// an error occurred!
			println!("Error: {:?}", e);
		}
	}
	println!("after build_log_store");
	
	//-------------------------------------------------------------------------------
	
	let event_mask = 
		EventMask::POINTER_MOTION_MASK |
		EventMask::BUTTON_PRESS_MASK |
		EventMask::BUTTON_RELEASE_MASK |
		EventMask::KEY_PRESS_MASK |
		EventMask::KEY_RELEASE_MASK |
		EventMask::SCROLL_MASK;

	drawing_area.set_can_focus(true);
	drawing_area.add_events(event_mask);
	let f_clone_1 = store_rc.clone();
	drawing_area.connect_event(move |x, y| handle_evt(&mut f_clone_1.clone().borrow_mut(), x, y));

	// establish a reasonable minimum view size
	drawing_area.set_size_request(200, 200);
	
	// https://gtk-rs.org/docs/gtk/trait.WidgetExt.html
	let f_clone_2 = store_rc.clone();
	drawing_area.connect_draw(move |x, y| draw(&mut f_clone_2.clone().borrow_mut(), x, y));
	
	let f_clone_3 = store_rc.clone();
	drawing_area.connect_scroll_event(move |x, y| handle_evt_scroll(&mut f_clone_3.clone().borrow_mut(), x, y));
	
	let f_clone_4 = store_rc.clone();
	drawing_area.connect_button_press_event(move |x, y| handle_evt_press(&mut f_clone_4.clone().borrow_mut(), x, y));
	
	let f_clone_5 = store_rc.clone();
	drawing_area.connect_button_release_event(move |x, y| handle_evt_release(&mut f_clone_5.clone().borrow_mut(), x, y));
	
	let f_clone_6 = store_rc.clone();
	drawing_area.connect_motion_notify_event(move |x, y| handle_evt_motion(&mut f_clone_6.clone().borrow_mut(), x, y));
	
	split_pane.pack_start(&drawing_area, true, true, 10);

    window.add(&split_pane);
    window.show_all();
}

fn gio_files_to_paths(gio_files : &[gio::File]) -> Vec<std::path::PathBuf> {
	let mut result = Vec::new();
	for gio_file in gio_files {
		result.push(gio_file.get_path().expect("Invalid file path"));
	}
	result
}

fn main() {
	
	// https://developer.gnome.org/CommandLine/
	// https://developer.gnome.org/GtkApplication/
	
    let application =
        gtk::Application::new(Some("com.github.BenjaminRi.Sherlog"), gio::ApplicationFlags::HANDLES_OPEN)
            .expect("Initialization failed...");
	
	// https://gtk-rs.org/docs/glib/struct.OptionFlags.html
	// https://gtk-rs.org/docs/glib/enum.OptionArg.html
	application.add_main_option(
		"CTest",
		glib::Char::new('c').unwrap(),
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
