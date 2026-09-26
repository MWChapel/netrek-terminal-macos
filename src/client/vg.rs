//! Vector drawing on top of tiny-skia (a Rust port of Skia's 2D rasterizer):
//! anti-aliased strokes with proper joins, dashes, circles, polygons,
//! gradients and text. The result is shipped to the terminal as an image.

use super::palette::Rgb;
use super::sixel::TextRenderer;
use tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Paint, PathBuilder, Pixmap, Point,
    RadialGradient, Rect, Shader, SpreadMode, Stroke, StrokeDash, Transform,
};

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pm: Pixmap,
}

fn color(c: Rgb, a: f32) -> Color {
    let u = |v: f32| (v.clamp(0.0, 255.0)) as u8;
    Color::from_rgba8(u(c[0]), u(c[1]), u(c[2]), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

fn paint(c: Rgb, a: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color(c, a));
    p.anti_alias = true;
    p
}

fn stroke(width: f32, dash: f32) -> Stroke {
    let mut s = Stroke { width, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
    if dash > 0.0 {
        s.dash = StrokeDash::new(vec![dash, dash], 0.0);
        s.line_cap = LineCap::Butt;
    }
    s
}

impl Canvas {
    pub fn new(w: i32, h: i32, bg: Rgb) -> Canvas {
        let mut pm = Pixmap::new(w.max(1) as u32, h.max(1) as u32).expect("pixmap");
        pm.fill(color(bg, 1.0));
        Canvas { w: w.max(1), h: h.max(1), pm }
    }

    /// Paint another canvas onto this one with its top-left corner at (x, y).
    pub fn draw_canvas(&mut self, other: &Canvas, x: i32, y: i32) {
        self.pm.draw_pixmap(x, y, other.pm.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
    }

    pub fn get(&self, x: i32, y: i32) -> Rgb {
        let p = self.pm.pixels()[(y * self.w + x) as usize];
        [p.red() as f32, p.green() as f32, p.blue() as f32]
    }

    pub fn polyline(&mut self, pts: &[(f32, f32)], closed: bool, width: f32, c: Rgb, a: f32, dash: f32) {
        if pts.len() < 2 || a <= 0.0 {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0, pts[0].1);
        for p in &pts[1..] {
            pb.line_to(p.0, p.1);
        }
        if closed {
            pb.close();
        }
        if let Some(path) = pb.finish() {
            self.pm.stroke_path(&path, &paint(c, a), &stroke(width, dash), Transform::identity(), None);
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, c: Rgb, a: f32, dash: f32) {
        self.polyline(&[(x0, y0), (x1, y1)], false, width, c, a, dash);
    }

    pub fn fill_poly(&mut self, pts: &[(f32, f32)], c: Rgb, a: f32) {
        if pts.len() < 3 || a <= 0.0 {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0, pts[0].1);
        for p in &pts[1..] {
            pb.line_to(p.0, p.1);
        }
        pb.close();
        if let Some(path) = pb.finish() {
            self.pm.fill_path(&path, &paint(c, a), FillRule::Winding, Transform::identity(), None);
        }
    }

    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, width: f32, c: Rgb, a: f32) {
        self.ring_dashed(cx, cy, r, width, c, a, 0.0);
    }

    pub fn ring_dashed(&mut self, cx: f32, cy: f32, r: f32, width: f32, c: Rgb, a: f32, dash: f32) {
        if r <= 0.0 || a <= 0.0 {
            return;
        }
        if let Some(path) = PathBuilder::from_circle(cx, cy, r) {
            self.pm.stroke_path(&path, &paint(c, a), &stroke(width, dash), Transform::identity(), None);
        }
    }

    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, c: Rgb, a: f32) {
        if r <= 0.0 || a <= 0.0 {
            return;
        }
        if let Some(path) = PathBuilder::from_circle(cx, cy, r) {
            self.pm.fill_path(&path, &paint(c, a), FillRule::Winding, Transform::identity(), None);
        }
    }

    /// Soft radial glow: color `c` at alpha `a` in the middle fading to nothing.
    pub fn glow(&mut self, cx: f32, cy: f32, r: f32, c: Rgb, a: f32) {
        if r <= 0.5 || a <= 0.0 {
            return;
        }
        let stops = vec![GradientStop::new(0.0, color(c, a)), GradientStop::new(1.0, color(c, 0.0))];
        let Some(shader) =
            RadialGradient::new(Point::from_xy(cx, cy), 0.0, Point::from_xy(cx, cy), r, stops, SpreadMode::Pad, Transform::identity())
        else {
            return;
        };
        let mut p = Paint::default();
        p.shader = shader;
        p.anti_alias = true;
        if let Some(path) = PathBuilder::from_circle(cx, cy, r) {
            self.pm.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
        }
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Rgb, a: f32) {
        if let Some(r) = Rect::from_xywh(x, y, w, h) {
            self.pm.fill_rect(r, &paint(c, a), Transform::identity(), None);
        }
    }

    /// Rounded rectangle outline or fill.
    pub fn round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, rad: f32, c: Rgb, a: f32, width: Option<f32>) {
        let r = rad.min(w / 2.0).min(h / 2.0);
        let mut pb = PathBuilder::new();
        pb.move_to(x + r, y);
        pb.line_to(x + w - r, y);
        pb.quad_to(x + w, y, x + w, y + r);
        pb.line_to(x + w, y + h - r);
        pb.quad_to(x + w, y + h, x + w - r, y + h);
        pb.line_to(x + r, y + h);
        pb.quad_to(x, y + h, x, y + h - r);
        pb.line_to(x, y + r);
        pb.quad_to(x, y, x + r, y);
        pb.close();
        let Some(path) = pb.finish() else { return };
        match width {
            Some(wd) => self.pm.stroke_path(&path, &paint(c, a), &stroke(wd, 0.0), Transform::identity(), None),
            None => self.pm.fill_path(&path, &paint(c, a), FillRule::Winding, Transform::identity(), None),
        }
    }

    /// Rectangle filled with a left-to-right gradient spanning `full_w` pixels,
    /// shown only up to width `w` (used for gauges).
    pub fn gradient_bar(&mut self, x: f32, y: f32, w: f32, h: f32, full_w: f32, stops: &[(f32, Rgb)]) {
        if w <= 0.0 {
            return;
        }
        let gs = stops.iter().map(|&(t, c)| GradientStop::new(t, color(c, 1.0))).collect();
        let shader = LinearGradient::new(
            Point::from_xy(x, y),
            Point::from_xy(x + full_w.max(1.0), y),
            gs,
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap_or(Shader::SolidColor(color(stops[0].1, 1.0)));
        let mut p = Paint::default();
        p.shader = shader;
        p.anti_alias = true;
        if let Some(r) = Rect::from_xywh(x, y, w, h) {
            self.pm.fill_rect(r, &p, Transform::identity(), None);
        }
    }

    pub fn dot(&mut self, x: f32, y: f32, c: Rgb, a: f32) {
        self.fill_rect(x.floor(), y.floor(), 1.0, 1.0, c, a);
    }

    fn blend_px(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h || a <= 0.0 {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let d = self.pm.data_mut();
        for k in 0..3 {
            let v = d[i + k] as f32;
            d[i + k] = (v + (c[k] - v) * a.min(1.0)).clamp(0.0, 255.0) as u8;
        }
    }

    pub fn text(&mut self, tr: &TextRenderer, x: f32, baseline: f32, s: &str, size: f32, c: Rgb) {
        tr.render(x, baseline, s, size, |px, py, cov| self.blend_px(px, py, c, cov));
    }

    pub fn text_centered(&mut self, tr: &TextRenderer, cx: f32, baseline: f32, s: &str, size: f32, c: Rgb) {
        let x = cx - tr.width(s, size) / 2.0;
        self.text(tr, x, baseline, s, size, c);
    }

    pub fn text_right(&mut self, tr: &TextRenderer, right: f32, baseline: f32, s: &str, size: f32, c: Rgb) {
        let x = right - tr.width(s, size);
        self.text(tr, x, baseline, s, size, c);
    }
}
