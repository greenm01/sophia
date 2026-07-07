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

    pub fn extend(&mut self, other: &Region) {
        self.rects
            .extend(other.rects.iter().copied().filter(|rect| !rect.is_empty()));
    }

    pub fn push(&mut self, rect: Rect) {
        if !rect.is_empty() {
            self.rects.push(rect);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_drops_empty_rectangles() {
        let mut region = Region::empty();
        region.push(Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 10,
        });
        region.push(Rect {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        });

        assert_eq!(region.rects.len(), 1);
    }
}
