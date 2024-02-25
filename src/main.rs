mod ansi;
mod buffer;
mod canvas;
mod color;
mod display;
mod error;
mod io;
mod key;
mod term;
mod window;

use window::{Window, Direction, Focus};
use buffer::Buffer;
use canvas::Point;
use color::Color;
use error::Error;
use key::{Key, Keyboard, Modifier};
use std::cell::RefCell;
use std::rc::Rc;
use term::Terminal;

fn main() -> Result<(), Error> {
    let mut buffer = Buffer::new()?;
    let _ = io::read_file("LICENSE", &mut buffer)?;
    let pos = buffer.size() / 2;
    buffer.set_pos(pos);
    let buf = Rc::new(RefCell::new(buffer));

    let mut win = Window::new(
        40,
        80,
        Color::new(color::BRIGHT_MAGENTA, 234),
        Point::new(5, 10),
        buf.clone());

    let (rows, cols) = term::size()?;
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
            Key::Up(Modifier::None) => {
                win.move_cursor(Direction::Up);
            }
            Key::Up(Modifier::Shift) => {
                win.move_cursor(Direction::PageUp);
            }
            Key::Down(Modifier::None) => {
                win.move_cursor(Direction::Down);
            }
            Key::Down(Modifier::Shift) => {
                win.move_cursor(Direction::PageDown);
            }
            Key::Left(Modifier::None) => {
                win.move_cursor(Direction::Left);
            }
            Key::Right(Modifier::None) => {
                win.move_cursor(Direction::Right);
            }
            Key::Control(12) => {
                win.align_cursor(Focus::Auto);
                win.redraw();
            }
            key => {
                println!("{:?}\r", key);
            }
        }
    }
    Ok(())
}
