use crate::model_internal::LogEntryExt;

pub struct ScrollBarVert {
	pub x : f64,
	pub y : f64,
	
	pub bar_padding : f64,
	pub bar_width : f64,
	pub bar_height : f64,
	
	pub thumb_x : f64,
	pub thumb_y : f64,
	pub thumb_margin : f64,
	pub thumb_width : f64,
	pub thumb_height : f64,
	pub thumb_rel_offset : f64,
	
	pub scroll_perc : f64,
}

pub struct LogStoreLinear {
	pub store : Vec::<LogEntryExt>,
	pub visible_lines : usize,
	pub cursor_pos : usize,
	pub mouse_down : bool,
	pub thumb_drag : bool,
	pub thumb_drag_x : f64,
	pub thumb_drag_y : f64,
	pub scroll_bar : ScrollBarVert,
}

impl LogStoreLinear {
	//pub fn filter_store(&mut self, filter : |&LogEntryExt| -> bool, active: bool) {
	pub fn filter_store(&mut self, filter : &Fn(&LogEntryExt) -> bool, active: bool) {
		let mut next_entry_id = 0;
		for entry in self.store.iter_mut() {
			if filter(entry) {
				entry.visible = active;
			}
			if entry.visible {
				entry.entry_id = next_entry_id;
				next_entry_id += 1;
			}
		}
	}
}
