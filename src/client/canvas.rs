//! Terminal drawing: a character-cell screen buffer that only repaints what
//! changed, with holes left for the pixel images.

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
