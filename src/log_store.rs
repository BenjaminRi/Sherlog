extern crate chrono;

use chrono::prelude::*;
use std::collections::HashSet;
use std::collections::HashMap;

use crate::model;

use crate::model_internal::LogEntryExt;

pub struct ScrollBarVert {
	pub x: f64,
	pub y: f64,

	pub bar_padding: f64,
	pub bar_width: f64,
	pub bar_height: f64,

	pub thumb_x: f64,
	pub thumb_y: f64,
	pub thumb_margin: f64,
	pub thumb_width: f64,
	pub thumb_height: f64,
	pub thumb_rel_offset: f64,

	pub scroll_perc: f64,
}

pub struct LogStoreLinear {
	pub store: Vec<LogEntryExt>,
	pub entry_count: usize, //entry_count <= store.len(), number of active items
	pub first_offset: usize, //first_offset < store.len(), offset of first active element in vec
	pub last_offset: usize, //last_offset < store.len(), offset of last active element in vec
	pub anchor_offset: Option<usize>, //anchor_offset < store.len(), offset of anchor element that aligns GUI on visibility changes
	
	pub show_crit: bool,
	pub show_err: bool,
	pub show_warn: bool,
	pub show_info: bool,
	pub show_dbg: bool,
	pub show_trace: bool,
	
	pub selected_single: HashSet<usize>,
	pub excluded_single: HashSet<usize>,
	pub selected_single_last: Option<usize>,
	pub selected_range: Option<(usize, usize)>,
	
	pub pressed_shift: bool,
	pub pressed_ctrl: bool,
	
	pub log_sources: HashMap<u32, String>,

	pub visible_lines: usize, //visible entries in GUI (i.e. number of rows your text viewport has)
	pub hover_line: Option<usize>, //line the mouse cursor hovers over, relative to viewport_offset
	pub viewport_offset: usize, //viewport_offset < store.len(), offset of GUI viewport
	pub mouse_down: bool,
	pub thumb_drag: bool,
	pub thumb_drag_x: f64,
	pub thumb_drag_y: f64,
	pub scroll_bar: ScrollBarVert,
	
	pub border_left: f64,
	pub border_top: f64,
	pub border_bottom: f64,
	pub line_spacing: f64,
	pub font_size: f64,
}

impl LogStoreLinear {
	pub fn rel_to_abs_offset(&self, rel_offset: usize) -> Option<usize> {
		for (offset, _) in self
			.store
			.iter()
			.enumerate() //offset in vector
			.skip(self.viewport_offset)
			.filter(|(_, x)| x.is_visible())
			.skip(rel_offset)
			.take(1)
		{
			return Some(offset);
		}
		return None;
	}
	
	pub fn abs_to_rel_offset(&self, abs_offset: usize) -> Option<usize> {
		for (i, (offset, _)) in self
			.store
			.iter()
			.enumerate() //offset in vector
			.skip(self.viewport_offset)
			.filter(|(_, x)| x.is_visible())
			.take(self.visible_lines)
			.enumerate() //index of filtered element
		{
			if offset == abs_offset {
				return Some(i);
			}
		}
		return None;
	}


	//pub fn filter_store(&mut self, filter : |&LogEntryExt| -> bool, active: bool) {
	pub fn filter_store(&mut self, filter: &dyn Fn(&LogEntryExt) -> bool, active: bool, mask: u8) {
		//Note: The code in this function must be fast. It is critical GUI code.
		//If this code is slow, then the user will have noticeable GUI lag.
		
		let (tmp_anchor_offset, rel_offset) = {
			if let Some(anchor_offset) = self.anchor_offset {
				if let Some(rel_offset) = self.abs_to_rel_offset(anchor_offset) {
					(anchor_offset, rel_offset)
				} else {
					//Anchor is not active in viewport
					//Test if anchor is between the lines of viewport
					let mut rel_offset = None;
					if let Some(mut abs_offset) = self.rel_to_abs_offset(0) {
						let mut prev_offset = abs_offset;
						for i in 1..self.visible_lines {
							//Note: The last element points to itself, so it's safe in all cases
							abs_offset = self.store[abs_offset].next_offset as usize;
							
							if anchor_offset >= prev_offset && anchor_offset <= abs_offset {
								rel_offset = Some(i);
								break;
							}
							prev_offset = abs_offset;
						}
					}
					if let Some(rel_offset) = rel_offset {
						//Hold anchor in place because anchor is between the lines of viewport
						(anchor_offset, rel_offset)
					} else {
						//Fallback: Align to the middle of the screen
						(anchor_offset, (std::cmp::max(1, self.visible_lines)-1)/2)
					}
				}
			} else if self.entry_count >= self.visible_lines {
				if let Some(mut abs_offset) = self.rel_to_abs_offset(0) {
					//Got top visible line; now advance anchor by half the visible lines
					let rel_offset = (std::cmp::max(1, self.visible_lines)-1)/2;
					for _ in 0..rel_offset {
						//Note: The last element points to itself, so it's safe in all cases
						abs_offset = self.store[abs_offset].next_offset as usize;
					}
					(abs_offset, rel_offset)
				} else {
					(0,0)
				}
			} else {
				//No anchor; less elements in store than viewport size; just reset to zero
				(0, 0)
			}
		};

		let mut next_entry_id = 0;

		let mut dummy = LogEntryExt {
			timestamp: DateTime::<Utc>::from_utc(
				NaiveDateTime::from_timestamp_opt(0, 0).unwrap(),
				Utc,
			),
			severity: model::LogLevel::Error,
			message: "Foo".to_string(),
			source_id: 0,
			visible: crate::model_internal::VISIBLE_ON,
			entry_id: 0,
			prev_offset: 0,
			next_offset: 0,
		};

		{
			let mut prev = &mut dummy;
			let mut prev_offset = 0;
			for (offset, entry) in self.store.iter_mut().enumerate() {
				if filter(entry) {
					if active {
						entry.visible &= !mask; //remove mask to show entry
					} else {
						entry.visible |= mask; //apply mask
					}
				}
				if entry.is_visible() {
					entry.entry_id = next_entry_id;
					next_entry_id += 1;

					prev.next_offset = offset as u32;
					entry.prev_offset = prev_offset;

					prev_offset = offset as u32;
					prev = entry;
				}
			}

			//Prev is the last element now:
			self.last_offset = prev_offset as usize;
			prev.next_offset = prev_offset; //Last element points to itself
		}

		self.entry_count = next_entry_id as usize; //Conveniently, we can use this as number of elements
		self.first_offset = dummy.next_offset as usize; //The element after the dummy is the first real element
		if self.store.len() > 0 {
			self.store[self.first_offset].prev_offset = self.first_offset as u32;
			//First element points to itself
		}
		
		if self.store.len() > 0 && self.store[tmp_anchor_offset].is_visible() {
			self.viewport_offset = tmp_anchor_offset;
			self.scroll(-(rel_offset as i64), self.visible_lines);
		} else {
			let mut found = false;
			for (offset, _) in self
				.store
				.iter()
				.enumerate() //offset in vector
				.skip(tmp_anchor_offset)
				.filter(|(_, x)| x.is_visible())
				.take(1)
			{
				self.viewport_offset = offset; 
				found = true;
			}
			if !found {
				self.viewport_offset = self.last_offset;
				println!("Not found {}!!", rel_offset);
			}
			self.scroll(-(rel_offset as i64), self.visible_lines);
		}
	}

	pub fn percentage_to_offset(&self, perc: f64, window_size: usize) -> Option<usize> {
		if perc < 0.0 || perc > 1.0 {
			return None;
		}
		if window_size == 0 {
			return None;
		}
		if self.entry_count == 0 {
			return None;
		}
		if window_size >= self.entry_count {
			return Some(0);
		}

		let entry_id = ((self.entry_count - window_size) as f64 * perc).round() as u32;
		for (offset, _) in self
				.store
				.iter()
				.enumerate()
				.skip(std::cmp::max(1, entry_id as usize) - 1)
				.filter(|(_, x)| x.is_visible() && x.entry_id == entry_id)
				.take(1) {
			return Some(offset);
		}

		unreachable!()
	}

	pub fn get_scroll_percentage(&self, window_size: usize) -> f64 {
		if self.entry_count == 0 {
			return 0.0; //Early exit to prevent getting nonexistent vec elements!
		}
		if self.entry_count <= window_size {
			return 0.0; //Early exit to prevent division by 0, negative percentage
		}
		let percentage = (self.store[self.viewport_offset].entry_id as f64)
			/ ((self.entry_count - window_size) as f64);
		if percentage > 1.0 {
			return 1.0; //clamp down if scrolled too far or window too large
		}
		return percentage;
	}

	//Returns false if nothing happened, returns true if viewport offset changed
	pub fn scroll(&mut self, lines: i64, window_size: usize) -> bool {
		if self.entry_count == 0 {
			return false; //Early exit to prevent getting nonexistent vec elements!
		}
		let viewport_offset_old = self.viewport_offset;
		let mut abs_lines = lines.abs();
		if lines < 0 {
			while abs_lines > 0 {
				let new_offset = self.store[self.viewport_offset].prev_offset as usize;
				if self.viewport_offset == new_offset {
					break; //reached end of list
				}
				self.viewport_offset = new_offset;
				abs_lines -= 1;
			}
		} else {
			if self.entry_count <= window_size {
				return false; //no scrolling down if window larger than number of rows
			}
			while abs_lines > 0 {
				if (self.entry_count - window_size) <= self.store[self.viewport_offset].entry_id as usize
				{
					break; //stop scrolling down, bottomed out window
				}

				let new_offset = self.store[self.viewport_offset].next_offset as usize;
				if self.viewport_offset == new_offset {
					break; //reached end of list
				}

				self.viewport_offset = new_offset;
				abs_lines -= 1;
			}
		}

		return viewport_offset_old != self.viewport_offset;
	}
}
