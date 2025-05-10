use cosmic_text::ttf_parser::OutlineBuilder;
use lyon::math::Point;
use lyon::path::Path;

pub(crate) struct LyonCommandEncoder {
    builder: lyon::path::builder::WithSvg<lyon::path::builder::Flattened<lyon::path::BuilderImpl>>,
}

impl LyonCommandEncoder {
    pub fn new() -> Self {
        // maximum distance between a curve and its approximation.
        let tolerance = 0.05;
        Self {
            builder: Path::builder().with_svg().flattened(tolerance),
        }
    }

    pub fn build_path(self) -> Path {
        self.builder.build()
    }
}

impl Default for LyonCommandEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl OutlineBuilder for LyonCommandEncoder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(Point::new(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(Point::new(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.builder
            .quadratic_bezier_to(Point::new(x1, y1), Point::new(x, y));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.builder
            .cubic_bezier_to(Point::new(x1, y1), Point::new(x2, y2), Point::new(x, y));
    }

    fn close(&mut self) {
        self.builder.close();
    }
}
