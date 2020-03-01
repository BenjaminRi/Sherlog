extern crate chrono;

use chrono::prelude::*;

use crate::model;

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
	pub entry_count : usize, //entry_count <= store.len(), number of active items
	pub first_offset : usize,
	pub last_offset : usize,
	
	pub visible_lines : usize, //visible entries in GUI
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
		//Note: The code in this function must be fast. It is critical GUI code.
		//If this code is slow, then the user will have noticeable GUI lag.
		
		let mut next_entry_id = 0;
		
		let mut dummy = LogEntryExt {
			timestamp: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp_opt(0, 0).unwrap(), Utc),
			severity: model::LogLevel::Error,
			message: "Foo".to_string(),
			source_id: 0,
			visible: false,
			entry_id : 0,
			prev_offset : 0,
			next_offset : 0,
		};
		
		{
			let mut offset = 0;
			let mut prev = &mut dummy;
			let mut prev_offset = 0;
			for entry in self.store.iter_mut() {
				if filter(entry) {
					entry.visible = active;
				}
				if entry.visible {
					entry.entry_id = next_entry_id;
					next_entry_id += 1;
					
					prev.next_offset = offset;
					entry.prev_offset = prev_offset;
					
					prev_offset = offset;
					prev = entry;
				}
				offset += 1;
			}
			
			//Prev is the last element now:
			self.last_offset = prev_offset as usize; 
			prev.next_offset = prev_offset; //Last element points to itself
		}
		
		self.entry_count = next_entry_id as usize; //Conveniently, we can use this as number of elements
		self.first_offset = dummy.next_offset as usize; //The element after the dummy is the first real element
		if self.store.len() > 0 {
			self.store[self.first_offset].prev_offset = self.first_offset as u32; //First element points to itself
		}
		
		self.cursor_pos = self.first_offset; //reset cursor pos. TODO: Implement smart logic here to not reset it every time.
	}
	
	pub fn percentage_to_offset(& self, perc : f64, window_size : usize) -> Option<usize> {
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
		let mut offset = 0;
		for entry in self.store.iter() {
			if entry.visible && entry.entry_id == entry_id{
				return Some(offset)
			}
			offset += 1;
		}
		
		unreachable!()
	}
	
	pub fn get_scroll_percentage(& self, window_size : usize) -> f64 {
		if self.entry_count == 0 {
			return 0.0; //Early exit to prevent getting nonexistent vec elements!
		}
		if self.entry_count <= window_size {
			return 0.0; //Early exit to prevent division by 0, negative percentage
		}
		let percentage = (self.store[self.cursor_pos].entry_id as f64) / ((self.entry_count - window_size) as f64);
		if percentage > 1.0 {
			return 1.0; //clamp down if scrolled too far or window too large
		}
		return percentage;
	}
	
	//Returns false if nothing happened, returns true if cursor changed
	pub fn scroll(&mut self, lines: i64, window_size : usize) -> bool {
		if self.entry_count == 0 {
			return false; //Early exit to prevent getting nonexistent vec elements!
		}
		let cursor_pos_old = self.cursor_pos;
		let mut abs_lines = lines.abs();
		if lines < 0 {
			while abs_lines > 0 {
				let new_offset = self.store[self.cursor_pos].prev_offset as usize;
				if self.cursor_pos == new_offset {
					break; //reached end of list
				}
				self.cursor_pos = new_offset;
				abs_lines -= 1;
			}
		} else {
			if self.entry_count <= window_size {
				return false; //no scrolling down if window larger than number of rows
			}
			while abs_lines > 0 {
				if (self.entry_count - window_size) <= self.store[self.cursor_pos].entry_id as usize{
					break; //stop scrolling down, bottomed out window
				}
				
				let new_offset = self.store[self.cursor_pos].next_offset as usize;
				if self.cursor_pos == new_offset {
					break; //reached end of list
				}
				
				self.cursor_pos = new_offset;
				abs_lines -= 1;
			}
		}
		
		return (cursor_pos_old != self.cursor_pos);
	}
}


/*
#[derive(Debug)]
struct Foo {
    idx: u8,
    next: u8,
}

fn main() {
  let mut foo: Vec<Foo> = (0..9).map(|a| Foo {idx: a, next: a}).collect();
  //foo.clear();
  println!("start: {:?}",foo);
  {
    let (first, rest) = foo.split_at_mut(1); //panics on empty input
    println!("first: {:?}", first);
    println!("rest: {:?}", rest);
    if let Some(mut pre) = first.first_mut() {
        for cur in rest {
            println!("- {:?} {:?}", pre, cur);
            if cur.idx % 3 == 0 {
                pre.next = cur.idx;
                pre = cur
            }
        }
    }
  }
  println!("finish: {:?}",foo);
  //std::process::exit(foo.iter().map(|a| a.a as i32).sum())
}
*/




/*
#[derive(Debug)]
struct Foo {
    a: u8
}

fn main() {
  let mut foo: Vec<Foo> = (1..10).map(|a| Foo {a}).collect();
  println!("start: {:?}",foo);
  {
	if foo.is_empty() { return }
    let (first, rest) = foo.split_at_mut(1);
    let mut pre;
    if let [pre_] = first {
        pre = pre_;
        for cur in rest {
            println!("- {:?} {:?}", pre, cur);
            cur.a+=pre.a;
            if 0 == cur.a % 3 {
                pre = cur
            }
        }
    }
  }
  println!("finish: {:?}",foo);
  //std::process::exit(foo.iter().map(|a| a.a as i32).sum())
}
*/

/*
#[derive(Debug)]
struct Foo {
    a: u8
}

fn main() {
  let mut foo: Vec<Foo> = (1..10).map(|a| Foo {a}).collect();
  println!("start: {:?}",foo);
  {
    if let [ref mut first, ref mut rest @ ..] = &mut foo[..] {
        let mut pre = first;
        for cur in rest {
            println!("- {:?} {:?}", pre, cur);
            cur.a+=pre.a;
            pre.a+=cur.a;
            if 0 == cur.a % 3 {
                pre = cur
            }
        }
    }
  }
  println!("finish: {:?}",foo);
  //std::process::exit(foo.iter().map(|a| a.a as i32).sum())
}
*/