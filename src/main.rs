mod ansi;
mod buffer;
mod canvas;
mod color;
mod display;
mod document;
mod error;
mod io;
mod key;
mod term;
mod window;

use buffer::Buffer;
use canvas::Point;
use color::Color;
use document::{Direction, Document, Focus};
use error::Error;
use key::{Key, Keyboard, Modifier};
use term::Terminal;
use window::Window;

fn main() -> Result<(), Error> {
    let mut buffer = Buffer::new()?;
    let _ = io::read_file("TEST", &mut buffer)?;
    let pos = buffer.size() / 2;
    buffer.set_pos(pos);

    let window = Window::new(
        Point::new(5, 10),
        40,
        70,
        Color::new(color::BRIGHT_MAGENTA, 234),
    );

    let mut doc = Document::new(buffer, window);

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
                doc.move_cursor(Direction::Up);
            }
            Key::Up(Modifier::Shift) => {
                doc.move_cursor(Direction::PageUp);
            }
            Key::Down(Modifier::None) => {
                doc.move_cursor(Direction::Down);
            }
            Key::Down(Modifier::Shift) => {
                doc.move_cursor(Direction::PageDown);
            }
            Key::Left(Modifier::None) => {
                doc.move_cursor(Direction::Left);
            }
            Key::Right(Modifier::None) => {
                doc.move_cursor(Direction::Right);
            }
            Key::Control(12) => {
                doc.align_cursor(Focus::Auto);
            }
            Key::Control(31) => {
                let pos = doc.buffer().get_pos();
                let line = doc
                    .buffer()
                    .forward_from(0)
                    .take(pos)
                    .filter(|&c| c == '\n')
                    .count();
                println!("\x1b[55;1H|line: {}|", line + 1);
            }
            key => {
                println!("{:?}\r", key);
            }
        }
    }
    Ok(())
}
