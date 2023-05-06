mod buffer;
mod error;
mod term;

use buffer::Buffer;
use error::Error;

use std::io::{self, Read, Write};

fn main() -> Result<(), Error> {
    let mut buf = Buffer::with_capacity(8)?;
    println!("buf: {:?}", buf);
    println!("buf capacity: {}", buf.capacity());
    println!("buf size: {}", buf.size());

    for c in "hello world".chars() {
        buf.insert(c)?;
    }
    println!("buf size: {}", buf.size());
    println!("buf: {:?}", buf);

    buf.insert_str("this is cool")?;
    buf.remove();
    println!("buf size: {}", buf.size());
    println!("buf: {:?}", buf);

    let old_term = term::init()?;
    let (rows, cols) = term::size()?;
    println!("rows: {}, cols: {}", rows, cols);

    let mut tty = io::stdin().bytes();
    loop {
        match tty.next().transpose()? {
            Some(b'\x04') => break,
            Some(c) => print!("{}", c as char),
            None => {
                if term::size_changed() {
                    let (r, c) = term::size()?;
                    print!("({}, {})", r, c);
                }
            }
        }
        io::stdout().flush()?
    }
    term::restore(&old_term)?;
    Ok(())
}
