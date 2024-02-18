mod ansi;
mod buffer;
mod canvas;
mod color;
mod error;
mod io;
mod key;
mod term;
mod window;

use crate::window::Window;
use buffer::Buffer;
use canvas::{Canvas, Cell, Point};
use color::Color;
use error::Error;
use key::{Key, Keyboard};
use std::cell::RefCell;
use std::rc::Rc;
use std::mem;
use term::Terminal;

fn main() -> Result<(), Error> {
    let mut buf = Buffer::new()?;
    let n = io::read_file("LICENSE", &mut buf)?;
    print_buffer(&buf);
    println!("read {} characters", n);

    println!("---");
    buf.set_pos(9866);
    for c in buf.forward() {
        print!("{}", c);
    }
    println!("---");
    buf.set_pos(951);
    for c in buf.backward() {
        print!("{}", c);
    }
    println!("---");

//    println!("size Color: {}", mem::size_of::<Color>());
//    println!("size Point: {}", mem::size_of::<Point>());
//    println!("size Cell: {}", mem::size_of::<Cell>());
//    return Ok(());

/*
    let mut lines = vec![0];
    for (pos, c) in buf.forward_from(0).index() {
        if c == '\n' {
            lines.push(pos + 1);
        }
    }
    println!("lines: {}", lines.len());
    for (l, pos) in lines.iter().enumerate() {
        println!("[{}] -> {}", l, pos);
    }

    // found: pos is beg of line: Ok(i): line # = i + 1
    let r = lines.binary_search(&9438);
    print!("search(9438): {:?}: line: ", r);
    println!("{}", r.unwrap() + 1);
    // not found: pos is not beg of line: Err(i): line # = i
    let r = lines.binary_search(&8900);
    print!("search(8900): {:?}: line: ", r);
    println!("{}", r.unwrap_err());
*/

    let pos = buf.size() / 2;
    //let pos = 8488;
    //let pos = buf.size();
    println!("setting pos: {}", pos);
    buf.set_pos(pos);
    println!("[{}]: {:?}", pos, buf.get(pos));
    let buffer = Rc::new(RefCell::new(buf));
    let mut win = Window::new(
        40,
        80,
        Color::new(color::BLACK, color::BLUE),
        Point::new(5, 10),
        buffer.clone());
    println!("{}2J", ansi::CSI);
    win.draw();

    let (rows, cols) = term::size()?;
//    println!("rows: {}, cols: {}", rows, cols);

    let term = Terminal::new()?;
    let mut keyb = Keyboard::new(term);

    loop {
        match keyb.read()? {
            Key::Control(4) => break,
            Key::None => {
                if term::size_changed() {
                    let (rows, cols) = term::size()?;
                    println!("rows: {}, cols: {}\r", rows, cols);
                }
            }
            key => {
                println!("{:?}\r", key);
            }
        }
    }
    Ok(())
}

fn print_buffer(buf: &Buffer) {
    println!("--- {:?} ---", buf);
    for c in buf.forward_from(0) {
        print!("{}", c);
    }
    println!("\n---");
}
