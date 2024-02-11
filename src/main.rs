mod ansi;
mod buffer;
mod canvas;
mod error;
mod io;
mod key;
mod term;
mod window;

use crate::window::Window;
use buffer::Buffer;
use canvas::{Canvas, Cell};
use error::Error;
use key::{Key, Keyboard};
use std::cell::RefCell;
use std::rc::Rc;
use term::Terminal;

fn main() -> Result<(), Error> {
    let mut buf = Buffer::new()?;
    let n = io::read_file("LICENSE", &mut buf)?;
    print_buffer(&buf);
    println!("read {} characters", n);

    let n = io::write_file("LICENSE-copy", &buf)?;
    println!("wrote {} bytes", n);
    return Ok(());

    let (rows, cols) = term::size()?;
    println!("rows: {}, cols: {}", rows, cols);

    let mut canvas = Canvas::new(4, 8);

    // experiment
    let mut c = b'a';
    canvas.fill_with(|| {
        let r = c;
        if c == b'z' {
            c = b'a';
        } else {
            c += 1;
        }
        Cell {
            value: r as char,
            fg: 3,
            bg: 0,
        }
    });
    //    canvas.fill(Cell { value: 'a', fg: 3, bg: 0 });
    for (p, c) in canvas.iter() {
        println!("{:?} = {:?}", p, c);
    }

    let mut front = Canvas::new(4, 8);
    let mut back = Canvas::new(4, 8);
    back.put(2, 2, Cell::new('k', 1, 2));
    let changes = front.reconcile(&back);
    println!("changes: {:?}", changes);

    let buffer = Rc::new(RefCell::new(buf));
    let mut window = Window::new(10, 20, buffer.clone());
    window.debug_init();
    window.debug_change_0();
    window.refresh();
    window.debug_change_1();
    window.refresh();
    //    window.repaint();

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
