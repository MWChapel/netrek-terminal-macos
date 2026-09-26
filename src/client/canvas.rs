//! Terminal drawing: a character-cell screen buffer that only repaints what
//! changed, plus a braille "pixel" canvas giving 2x4 dots per character cell.

use crossterm::style::{Attribute, Color, Print, SetAttribute, SetBackgroundColor, SetForegroundColor};
use crossterm::{cursor, queue};
use std::io::{self, Write};

#[derive(Clone, Copy, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
}

const BLANK: Cell = Cell { ch: ' ', fg: Color::Reset, bg: Color::Reset, bold: false };

pub struct Screen {
    pub w: u16,
    pub h: u16,
    cells: Vec<Cell>,
    prev: Vec<Cell>,
    force: bool,
    /// Regions covered by pixel images; text output skips these cells.
    pub masks: Vec<(i32, i32, i32, i32)>,
}

impl Screen {
    pub fn new(w: u16, h: u16) -> Screen {
        let n = w as usize * h as usize;
        Screen { w, h, cells: vec![BLANK; n], prev: vec![BLANK; n], force: true, masks: Vec::new() }
    }

    /// Rewrite every cell on the next flush (e.g. after an image covered them).
    pub fn repaint(&mut self) {
        self.force = true;
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        *self = Screen::new(w, h);
    }

    pub fn clear(&mut self) {
        self.cells.fill(BLANK);
        self.masks.clear();
    }

    fn masked(&self, x: i32, y: i32) -> bool {
        self.masks.iter().any(|&(mx, my, mw, mh)| x >= mx && y >= my && x < mx + mw && y < my + mh)
    }

    pub fn put(&mut self, x: i32, y: i32, ch: char, fg: Color, bold: bool) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        self.cells[y as usize * self.w as usize + x as usize] = Cell { ch, fg, bg: Color::Reset, bold };
    }

    pub fn put_cell(&mut self, x: i32, y: i32, ch: char, fg: Color, bg: Color, bold: bool) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        self.cells[y as usize * self.w as usize + x as usize] = Cell { ch, fg, bg, bold };
    }

    pub fn text(&mut self, x: i32, y: i32, s: &str, fg: Color, bold: bool) {
        for (k, ch) in s.chars().enumerate() {
            self.put(x + k as i32, y, ch, fg, bold);
        }
    }

    /// Like `text`, but never writes past column `max_x`.
    pub fn text_clip(&mut self, x: i32, y: i32, s: &str, fg: Color, bold: bool, max_x: i32) {
        for (k, ch) in s.chars().enumerate() {
            if x + k as i32 > max_x {
                break;
            }
            self.put(x + k as i32, y, ch, fg, bold);
        }
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, ch: char, fg: Color) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, ch, fg, false);
            }
        }
    }

    /// Box with a title in the top border.
    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, title: &str, fg: Color) {
        if w < 2 || h < 2 {
            return;
        }
        for xx in x + 1..x + w - 1 {
            self.put(xx, y, '─', fg, false);
            self.put(xx, y + h - 1, '─', fg, false);
        }
        for yy in y + 1..y + h - 1 {
            self.put(x, yy, '│', fg, false);
            self.put(x + w - 1, yy, '│', fg, false);
        }
        self.put(x, y, '╭', fg, false);
        self.put(x + w - 1, y, '╮', fg, false);
        self.put(x, y + h - 1, '╰', fg, false);
        self.put(x + w - 1, y + h - 1, '╯', fg, false);
        if !title.is_empty() {
            self.text_clip(x + 2, y, &format!(" {} ", title), fg, true, x + w - 3);
        }
    }

    pub fn flush(&mut self, out: &mut impl Write) -> io::Result<()> {
        let mut cur_fg: Option<Color> = None;
        let mut cur_bg: Option<Color> = None;
        let mut cur_bold = false;
        let mut last_pos: Option<(u16, u16)> = None;
        queue!(out, SetAttribute(Attribute::Reset))?;
        for y in 0..self.h {
            for x in 0..self.w {
                let i = y as usize * self.w as usize + x as usize;
                let c = self.cells[i];
                if (!self.force && self.prev[i] == c) || self.masked(x as i32, y as i32) {
                    continue;
                }
                if last_pos != Some((x, y)) {
                    queue!(out, cursor::MoveTo(x, y))?;
                }
                if c.bold != cur_bold {
                    if c.bold {
                        queue!(out, SetAttribute(Attribute::Bold))?;
                    } else {
                        queue!(out, SetAttribute(Attribute::NormalIntensity))?;
                    }
                    cur_bold = c.bold;
                    cur_fg = None;
                    cur_bg = None;
                }
                if cur_bg != Some(c.bg) {
                    queue!(out, SetBackgroundColor(c.bg))?;
                    cur_bg = Some(c.bg);
                }
                if cur_fg != Some(c.fg) {
                    queue!(out, SetForegroundColor(c.fg))?;
                    cur_fg = Some(c.fg);
                }
                queue!(out, Print(c.ch))?;
                last_pos = Some((x + 1, y));
            }
        }
        queue!(out, SetAttribute(Attribute::Reset))?;
        self.prev.copy_from_slice(&self.cells);
        self.force = false;
        out.flush()
    }
}

/// A braille-dot canvas mapped onto a rectangle of screen cells.
pub struct Braille {
    pub cx: i32,
    pub cy: i32,
    pub cw: i32,
    pub ch: i32,
    bits: Vec<u8>,
    color: Vec<Color>,
    prio: Vec<u8>,
    bold: Vec<bool>,
}

impl Braille {
    pub fn new(cx: i32, cy: i32, cw: i32, ch: i32) -> Braille {
        let n = (cw.max(0) * ch.max(0)) as usize;
        Braille { cx, cy, cw, ch, bits: vec![0; n], color: vec![Color::Reset; n], prio: vec![0; n], bold: vec![false; n] }
    }

    /// Width and height in dots.
    pub fn dots(&self) -> (i32, i32) {
        (self.cw * 2, self.ch * 4)
    }

    /// Set a dot. Where several colors share a cell, the higher priority wins.
    pub fn dot(&mut self, x: i32, y: i32, color: Color, prio: u8) {
        if x < 0 || y < 0 || x >= self.cw * 2 || y >= self.ch * 4 {
            return;
        }
        let i = ((y / 4) * self.cw + x / 2) as usize;
        const MAP: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];
        self.bits[i] |= MAP[(x % 2) as usize][(y % 4) as usize];
        if prio >= self.prio[i] {
            self.prio[i] = prio;
            self.color[i] = color;
            self.bold[i] = prio >= 5;
        }
    }

    pub fn dotf(&mut self, x: f64, y: f64, color: Color, prio: u8) {
        self.dot(x.floor() as i32, y.floor() as i32, color, prio);
    }

    pub fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, color: Color, prio: u8) {
        self.line_pattern(x0, y0, x1, y1, color, prio, 1);
    }

    /// Line drawing every `step`-th dot (step 2+ gives a dotted line).
    pub fn line_pattern(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, color: Color, prio: u8, step: usize) {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let n = dx.abs().max(dy.abs()).ceil().max(1.0).min(4000.0) as usize;
        for k in (0..=n).step_by(step.max(1)) {
            let t = k as f64 / n as f64;
            self.dotf(x0 + dx * t, y0 + dy * t, color, prio);
        }
    }

    pub fn circle(&mut self, x: f64, y: f64, r: f64, color: Color, prio: u8) {
        self.arc(x, y, r, color, prio, 1);
    }

    pub fn arc(&mut self, x: f64, y: f64, r: f64, color: Color, prio: u8, step: usize) {
        if r < 0.6 {
            self.dotf(x, y, color, prio);
            return;
        }
        let n = ((r * 7.0) as usize).clamp(8, 400);
        for k in (0..n).step_by(step.max(1)) {
            let a = k as f64 / n as f64 * std::f64::consts::TAU;
            self.dotf(x + r * a.cos(), y + r * a.sin(), color, prio);
        }
    }

    pub fn disc(&mut self, x: f64, y: f64, r: f64, color: Color, prio: u8) {
        let ri = r.ceil() as i32;
        for yy in -ri..=ri {
            for xx in -ri..=ri {
                if (xx * xx + yy * yy) as f64 <= r * r {
                    self.dot(x as i32 + xx, y as i32 + yy, color, prio);
                }
            }
        }
    }

    pub fn blit(&self, screen: &mut Screen) {
        for yy in 0..self.ch {
            for xx in 0..self.cw {
                let i = (yy * self.cw + xx) as usize;
                if self.bits[i] != 0 {
                    let ch = char::from_u32(0x2800 + self.bits[i] as u32).unwrap_or(' ');
                    screen.put(self.cx + xx, self.cy + yy, ch, self.color[i], self.bold[i]);
                }
            }
        }
    }
}
