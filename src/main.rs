mod error;
mod key;
mod term;

use error::Error;
use key::{Key, Keyboard};
use term::Terminal;

fn main() -> Result<(), Error> {
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
