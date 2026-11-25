// A sample for the Rust grammar library: the definitions below are listed in `sample.expected`.

use std::fmt;

pub struct Point {
    pub x: i32,
    pub y: i32,
}

pub enum Shape {
    Dot(Point),
    Empty,
}

pub fn origin() -> Point {
    Point { x: 0, y: 0 }
}

fn describe(shape: &Shape) -> &'static str {
    match shape {
        Shape::Dot(_) => "dot",
        Shape::Empty => "empty",
    }
}
