use core::f32;

use crate::{color::ColorU, geometry::rect::RectF};

pub struct Scene {
    scale_factor: f32,
    layers: Vec<Layer>,
    active_layer_stack: Vec<usize>
}

#[derive(Default)]
pub struct Layer {
    clip_bounds: Option<RectF>,
    quads: Vec<Quad>
}

#[derive(Default, Debug)]
pub struct Quad {
    pub bounds: RectF,
    pub background: Option<ColorU>,
    pub border: Border,
    pub corner_radius: f32
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Border {
    pub width: f32,
    pub color: Option<ColorU>,

    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool
}

impl Scene {
    pub fn new(scale_factor: f32) -> Self {
        Scene {
            scale_factor,

            layers: vec![Layer::default()],

            active_layer_stack: vec![0]
        }
    }

    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    pub fn layers(&self) -> &[Layer] {
        self.layers.as_slice()
    }

    // pub fn push_layer(&mut self, clip_bounds: Option<RectF>) {
    //
    // }
    //
    // pub fn pop_layer(&mut self) {
    //     assert!(self.active_layer_stack.len() > 1);
    //
    //     self.active_layer_stack.pop();
    // }

    pub fn push_quad(&mut self, quad: Quad) {
        self.active_layer().push_quad(quad)
    }

    fn active_layer(&mut self) -> &mut Layer {
        &mut self.layers[*self.active_layer_stack.last().unwrap()]
    }
}

impl Layer {
    fn push_quad(&mut self, quad: Quad) {
        self.quads.push(quad);
    }

    pub fn quads(&self) -> &[Quad] {
        self.quads.as_slice()
    }
}

impl Border {
    pub fn new(width: f32, color: impl Into<ColorU>) -> Self {
        Self {
            width,
            color: Some(color.into()),

            top: false,
            left: false,

            bottom: false,
            right: false
        }
    }

    pub fn all(width: f32, color: impl Into<ColorU>) -> Self {
        Self {
            width,
            color: Some(color.into()),

            top: true,
            left: true,

            bottom: true,
            right: true
        }
    }

    pub fn top(width: f32, color: impl Into<ColorU>) -> Self {
        let mut border = Self::new(width, color);
        
        border.top = true;
        border
    }

    pub fn left(width: f32, color: impl Into<ColorU>) -> Self {
        let mut border = Self::new(width, color);

        border.left = true;
        border
    }

    pub fn bottom(width: f32, color: impl Into<ColorU>) -> Self {
        let mut border = Self::new(width, color);

        border.bottom = true;
        border
    }

    pub fn right(width: f32, color: impl Into<ColorU>) -> Self {
        let mut border = Self::new(width, color);

        border.right = true;
        border
    }

    pub fn with_sides(mut self, top: bool, left: bool, bottom: bool, right: bool) -> Self {
        self.top = top;
        self.left = left;

        self.bottom = bottom;
        self.right = right;

        self
    }

    fn all_sides(&self) -> bool {
        self.top && self.left && self.bottom && self.right
    }
}