//! The field manual, shown in place of the player list (G): a players' and
//! strategy guide to the galaxy, paged with [ ] and { }.
//!
//! The text lives in guide.txt, in a small markup: "# " a chapter, "## " a
//! section, "- " a bullet, "> " a note, "@ key | text" a key row, and any
//! other line a paragraph.

use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Chapter,
    Section,
    Para,
    Bullet,
    Note,
    /// A key (or name) and what it does.
    Key(String),
}

#[derive(Clone, Debug)]
pub struct Block {
    pub kind: Kind,
    pub text: String,
}

const TEXT: &str = include_str!("guide.txt");

pub fn blocks() -> &'static [Block] {
    static BLOCKS: OnceLock<Vec<Block>> = OnceLock::new();
    BLOCKS.get_or_init(|| {
        TEXT.lines()
            .map(str::trim_end)
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let (kind, text) = if let Some(t) = l.strip_prefix("## ") {
                    (Kind::Section, t)
                } else if let Some(t) = l.strip_prefix("# ") {
                    (Kind::Chapter, t)
                } else if let Some(t) = l.strip_prefix("- ") {
                    (Kind::Bullet, t)
                } else if let Some(t) = l.strip_prefix("> ") {
                    (Kind::Note, t)
                } else if let Some(t) = l.strip_prefix("@ ") {
                    let (k, d) = t.split_once(" | ").unwrap_or((t, ""));
                    return Block { kind: Kind::Key(k.to_string()), text: d.to_string() };
                } else {
                    (Kind::Para, l)
                };
                Block { kind, text: text.to_string() }
            })
            .collect()
    })
}

/// Number of chapters.
pub fn chapters() -> usize {
    blocks().iter().filter(|b| b.kind == Kind::Chapter).count()
}

/// Which chapter (1-based) block `pos` is in.
pub fn chapter_at(pos: usize) -> usize {
    blocks()[..=pos.min(blocks().len() - 1)].iter().filter(|b| b.kind == Kind::Chapter).count().max(1)
}

/// The start of the next (`dir` > 0) or previous chapter from `pos`.
pub fn jump(pos: usize, dir: i32) -> usize {
    let b = blocks();
    let starts: Vec<usize> = (0..b.len()).filter(|&k| b[k].kind == Kind::Chapter).collect();
    if dir > 0 {
        starts.iter().copied().find(|&k| k > pos).unwrap_or(pos)
    } else {
        // Back to the start of this chapter, or the one before if already there.
        starts.iter().copied().rev().find(|&k| k < pos).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_parses_into_chapters() {
        assert_eq!(chapters(), 13);
        assert_eq!(blocks()[0].kind, Kind::Chapter);
        assert!(blocks().iter().any(|b| matches!(&b.kind, Kind::Key(k) if k == "G")));
        let second = jump(0, 1);
        assert_eq!(chapter_at(second), 2);
        assert_eq!(jump(second, -1), 0);
        assert_eq!(jump(second + 1, -1), second);
    }
}
