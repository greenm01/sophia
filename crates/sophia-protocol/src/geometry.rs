#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const fn is_empty(self) -> bool {
        self.width <= 0 || self.height <= 0
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Region {
    pub rects: Vec<Rect>,
}

impl Region {
    pub fn empty() -> Self {
        Self { rects: Vec::new() }
    }

    pub fn single(rect: Rect) -> Self {
        if rect.is_empty() {
            Self::empty()
        } else {
            Self { rects: vec![rect] }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub matrix: [f32; 9],
}

impl Transform {
    pub const IDENTITY: Self = Self {
        matrix: [
            1.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, //
            0.0, 0.0, 1.0,
        ],
    };
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}
