mod buffer;
mod canvas;
mod error;
mod key;
mod term;

use buffer::Buffer;
use canvas::{Canvas, Cell};
use error::Error;
use key::{Key, Keyboard};
use term::Terminal;

fn main() -> Result<(), Error> {
    let mut buf = Buffer::with_capacity(10)?;
    print_buffer(&buf);

    for c in 'a'..='z' {
        buf.insert(c)?;
    }
    print_buffer(&buf);

    buf.set_pos(buf.size() / 2);
    print_buffer(&buf);

    buf.insert_chars(&vec!['+', '-', '*', '/'])?;
    print_buffer(&buf);

    buf.set_pos(2);
    let c = buf.delete();
    println!("delete: {:?}", c);
    print_buffer(&buf);

    buf.set_pos(12);
    let cs = buf.delete_chars(4);
    println!("delete_chars: {:?}", cs);
    print_buffer(&buf);

    let (rows, cols) = term::size()?;
    println!("rows: {}, cols: {}", rows, cols);

    let canvas = Canvas::new(4, 8);
    for (p, c) in canvas.iter() {
        println!("{:?} = {:?}", p, c);
    }

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
    for c in buf.forward_iter(0) {
        print!("{}", c);
    }
    println!("\n---");
}
