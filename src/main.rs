mod buffer;
mod error;
mod key;
mod term;

use buffer::Buffer;
use error::Error;
use key::{Key, Keyboard};
use term::Terminal;

fn main() -> Result<(), Error> {
    let mut buf = Buffer::with_capacity(10)?;
    print_buffer(&buf);

    for c in 'a'..='z' {
        buf.insert(0, c);
    }
    print_buffer(&buf);

    buf.insert_chars(buf.size() / 2, &vec!['+', '-', '*', '/']);
    print_buffer(&buf);

    let c = buf.delete(2);
    println!("delete(2): {}", c);
    print_buffer(&buf);

    let cs = buf.delete_chars(12, 16);
    println!("delete_chars(12, 16): {:?}", cs);
    print_buffer(&buf);

    let (rows, cols) = term::size()?;
    println!("rows: {}, cols: {}", rows, cols);

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
    for c in buf.forward() {
        print!("{}", c);
    }
    println!("\n---");
}
