use std::convert::From;

#[derive(Copy, Clone, Debug)]
pub struct Point {
    pub row: usize,
    pub col: usize,
}

impl Point {
    pub fn new(row: usize, col: usize) -> Point {
        Point { row, col }
    }
}

impl From<(usize, usize)> for Point {
    fn from(point: (usize, usize)) -> Point {
        Point::new(point.0, point.1)
    }
}

#[derive(Clone, Debug)]
pub struct Cell {
    value: char,
    fg: u8,
    bg: u8,
}

impl Default for Cell {
    fn default() -> Cell {
        Cell {
            value: '\0',
            fg: 0,
            bg: 0,
        }
    }
}

pub struct Canvas {
    rows: usize,
    cols: usize,
    content: Vec<Cell>,
}

pub struct Iter<'a> {
    canvas: &'a Canvas,
    row: usize,
    col: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (Point, &'a Cell);

    fn next(&mut self) -> Option<Self::Item> {
        if self.row < self.canvas.rows {
            let point = Point::new(self.row, self.col);
            self.col += 1;
            if self.col == self.canvas.cols {
                self.row += 1;
                self.col = 0;
            }
            Some((point, &self.canvas.content[point.row * point.col]))
        } else {
            None
        }
    }
}

impl Canvas {
    pub fn new(rows: usize, cols: usize) -> Canvas {
        assert!(rows > 0);
        assert!(cols > 0);
        Canvas {
            rows,
            cols,
            content: vec![Cell::default(); rows * cols],
        }
    }

    pub fn iter(&self) -> Iter<'_> {
        Iter {
            canvas: &self,
            row: 0,
            col: 0,
        }
    }
}
