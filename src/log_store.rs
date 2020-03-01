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
		
		let mut dummy = LogEntryExt {
			timestamp: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp_opt(0, 0).unwrap(), Utc),
			severity: model::LogLevel::Error,
			message: "Foo".to_string(),
			source_id: 0,
			visible: false,
			entry_id : 0, //First element points to itself (0), dummy is later discarded
			prev_id : 0,
			next_id : 0,
		};
		let mut prev = &mut dummy;
		for entry in self.store.iter_mut() {
			if filter(entry) {
				entry.visible = active;
			}
			if entry.visible {
				entry.entry_id = next_entry_id;
				next_entry_id += 1;
				
				prev.next_id = entry.entry_id;
				entry.prev_id = prev.entry_id;
			}
			prev = entry;
		}
		
		prev.next_id = prev.entry_id; //Last element points to itself
		
		//next_entry_id == total number of visible entries
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