//! Sends display instructions to terminal.

use crate::ansi;
use crate::canvas::{Cell, Point};
use std::io::{self, Write};

pub struct Display {
    rows: u32,
    cols: u32,
    origin: Point,
    out: String,
}

impl Display {
    pub fn new(rows: u32, cols: u32, origin: Point) -> Display {
        assert!(rows > 0);
        assert!(cols > 0);

        Display {
            rows,
            cols,
            origin,
            out: String::new(),
        }
    }

    pub fn send(&mut self) {
        if self.out.len() > 0 {
            print!("{}", self.out);
            let _ = io::stdout().flush();
            self.out.clear();
        }
    }

//    pub fn write_cell(&mut self, p: Point, cell: Cell) {
//        self.write_cursor(p);
//        self.out.push_str(ansi::set_color(cell.color).as_str());
//        self.out.push(cell.value);
//    }

    pub fn write_cell(&mut self, p: Point, cell: Cell, last: Option<(Point, Cell)>) {
        match last {
            Some((prev_p, prev_cell)) => {
                if p.row != prev_p.row || p.col != prev_p.col + 1 {
                    self.write_cursor(p);
                }
                if cell.color != prev_cell.color {
                    self.out.push_str(ansi::set_color(cell.color).as_str());
                }
            }
            None => {
                self.write_cursor(p);
                self.out.push_str(ansi::set_color(cell.color).as_str());
            }
        }
        self.out.push(cell.value);
    }

//    pub fn write_cell_optimized(&mut self, p: Point, cell: Cell, prev_p: Point, prev_cell: Cell) {
//        if p.row != prev_p.row || p.col != prev_p.col + 1 {
//            self.write_cursor(p);
//        }
//        if cell.color != prev_cell.color {
//            self.out.push_str(ansi::set_color(cell.color).as_str());
//        }
//        self.out.push(cell.value);
//    }

    pub fn write_cursor(&mut self, cursor: Point) {
        self.out.push_str(ansi::set_cursor(self.origin + cursor).as_str());
    }
}
