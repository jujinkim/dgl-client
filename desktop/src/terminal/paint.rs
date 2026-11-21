use super::fonts::TerminalFace;
use alacritty_terminal::{
    Term,
    grid::Indexed,
    term::cell::{Cell, Flags},
};
use gpui::{FontWeight, Hsla, LineLayout, Pixels, Point, SharedString, TextRun, Window, point, px};
use std::{collections::HashMap, sync::Arc};

/// Most distinct non-ASCII cells kept shaped before the cache starts over.
const MAX_SHAPED: usize = 4096;

/// Shaped glyphs of single cells, kept across frames. A screen repeats a small set of
/// characters, so each one is shaped once per face and size instead of once per cell
/// on every frame.
#[derive(Default)]
pub(super) struct GlyphCache {
    family: SharedString,
    font_size: f32,
    /// Printable ASCII, regular then bold: the common case needs no hashing.
    ascii: Vec<Option<Arc<LineLayout>>>,
    other: HashMap<(char, bool), Arc<LineLayout>>,
}
impl GlyphCache {
    pub fn layout(
        &mut self,
        window: &Window,
        face: &TerminalFace,
        font_size: f32,
        c: char,
        zerowidth: Option<&[char]>,
        bold: bool,
    ) -> Arc<LineLayout> {
        if self.font_size != font_size || self.family != face.family || self.ascii.is_empty() {
            self.family = face.family.clone();
            self.font_size = font_size;
            self.ascii = vec![None; 2 * ASCII];
            self.other.clear();
        }
        let shape = |text: &str| {
            let mut font = face.font();
            if bold {
                font.weight = FontWeight::BOLD;
            }
            window.text_system().layout_line(
                text,
                px(font_size),
                &[TextRun {
                    len: text.len(),
                    font,
                    color: Hsla::default(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            )
        };
        let mut utf8 = [0; 4];
        match (c, zerowidth) {
            (' '..='~', None) => {
                let slot = &mut self.ascii[c as usize - ' ' as usize + bold as usize * ASCII];
                slot.get_or_insert_with(|| shape(c.encode_utf8(&mut utf8)))
                    .clone()
            }
            (_, None) => {
                if self.other.len() >= MAX_SHAPED && !self.other.contains_key(&(c, bold)) {
                    self.other.clear();
                }
                self.other
                    .entry((c, bold))
                    .or_insert_with(|| shape(c.encode_utf8(&mut utf8)))
                    .clone()
            }
            // Combining sequences are rare and unbounded: the frame's own cache holds them.
            (_, Some(marks)) => {
                let mut text = c.to_string();
                text.extend(marks);
                shape(&text)
            }
        }
    }
}
/// Count of printable ASCII characters, space to tilde.
const ASCII: usize = 95;

/// Paints one cell's glyphs with the cell's top-left corner at `origin`, vertically
/// centred in `line_height` like a shaped line.
pub(super) fn paint_glyphs(
    layout: &LineLayout,
    origin: Point<Pixels>,
    line_height: Pixels,
    color: Hsla,
    window: &mut Window,
) {
    let baseline = origin.y + (line_height - layout.ascent - layout.descent) / 2. + layout.ascent;
    for run in &layout.runs {
        for glyph in &run.glyphs {
            let at = point(origin.x + glyph.position.x, baseline);
            let _ = if glyph.is_emoji {
                window.paint_emoji(at, run.font_id, glyph.id, layout.font_size)
            } else {
                window.paint_glyph(at, run.font_id, glyph.id, layout.font_size, color)
            };
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CellLayer {
    Background,
    Glyph,
}

pub(super) fn paint_grid<T>(term: &Term<T>, mut paint: impl FnMut(CellLayer, Indexed<&Cell>)) {
    // Wide glyphs extend into their spacer cell. Paint every background first
    // so the spacer (or the next row) cannot cover an already painted glyph.
    for layer in [CellLayer::Background, CellLayer::Glyph] {
        for indexed in term.grid().display_iter() {
            if layer == CellLayer::Glyph
                && (indexed.cell.c == ' '
                    || indexed
                        .cell
                        .flags
                        .intersects(Flags::WIDE_CHAR_SPACER | Flags::HIDDEN))
            {
                continue;
            }
            paint(layer, indexed);
        }
    }
}

/// Adjacent cells of one row with the same key, painted as one quad.
#[derive(Debug, PartialEq)]
pub(super) struct Run<K> {
    pub row: i32,
    pub start: usize,
    pub end: usize,
    pub key: K,
}

pub(super) struct Runs<K>(Option<Run<K>>);

impl<K: PartialEq> Runs<K> {
    pub fn new() -> Self {
        Self(None)
    }
    /// Adds the next cell in row-major order; `None` paints nothing. Returns the
    /// previous run once this cell does not continue it.
    pub fn push(&mut self, row: i32, column: usize, key: Option<K>) -> Option<Run<K>> {
        if let (Some(run), Some(k)) = (&mut self.0, &key)
            && run.row == row
            && run.end == column
            && run.key == *k
        {
            run.end += 1;
            return None;
        }
        std::mem::replace(
            &mut self.0,
            key.map(|key| Run {
                row,
                start: column,
                end: column + 1,
                key,
            }),
        )
    }
    pub fn finish(&mut self) -> Option<Run<K>> {
        self.0.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_join_same_key_neighbours_within_a_row_only() {
        let mut runs = Runs::new();
        let mut out = vec![];
        for (row, column, key) in [
            (0, 0, Some('a')),
            (0, 1, Some('a')),
            (0, 2, None),
            (0, 3, Some('a')),
            (0, 4, Some('b')),
            (1, 0, Some('b')),
            (1, 1, Some('b')),
        ] {
            out.extend(runs.push(row, column, key));
        }
        out.extend(runs.finish());
        let run = |row, start, end, key| Run {
            row,
            start,
            end,
            key,
        };
        assert_eq!(
            out,
            [
                run(0, 0, 2, 'a'),
                run(0, 3, 4, 'a'),
                run(0, 4, 5, 'b'),
                run(1, 0, 2, 'b')
            ]
        );
        assert_eq!(runs.finish(), None);
    }
    use alacritty_terminal::{
        event::VoidListener,
        term::{Config, test::TermSize},
        vte::ansi::Processor,
    };

    #[gpui::test]
    fn glyph_cache_shapes_each_cell_once_per_face_and_size(cx: &mut gpui::TestAppContext) {
        let ui = cx.add_empty_window();
        ui.update(|window, _| {
            let face = TerminalFace::default();
            let mut cache = GlyphCache::default();
            for (c, bold) in [('a', false), ('a', true), ('한', false), ('─', true)] {
                let first = cache.layout(window, &face, 14., c, None, bold);
                let again = cache.layout(window, &face, 14., c, None, bold);
                assert!(Arc::ptr_eq(&first, &again), "{c} bold={bold}");
                assert_eq!(first.font_size, px(14.));
            }
            // Weight is part of the key: regular and bold fill separate slots.
            assert_eq!(cache.ascii.iter().flatten().count(), 2);
            assert_eq!(cache.other.len(), 2);
            // Combining sequences are shaped as one text and never stored.
            let stored = cache.other.len();
            let combined = cache.layout(window, &face, 14., 'e', Some(&['\u{301}']), false);
            assert_eq!(combined.len, "e\u{301}".len());
            assert_eq!(cache.other.len(), stored);
            // Another size or family starts over.
            assert_eq!(
                cache.layout(window, &face, 18., 'a', None, false).font_size,
                px(18.)
            );
            assert!(cache.other.is_empty());
            let other = TerminalFace {
                family: "Other".into(),
                ..face.clone()
            };
            cache.layout(window, &face, 18., '한', None, false);
            cache.layout(window, &other, 18., 'a', None, false);
            assert!(cache.other.is_empty());
            // The non-ASCII store is bounded.
            for c in ('가'..).take(MAX_SHAPED + 1) {
                cache.layout(window, &other, 18., c, None, false);
            }
            assert_eq!(cache.other.len(), 1);
        });
    }

    #[test]
    fn wide_glyph_coverage_survives_neighboring_cell_backgrounds() {
        // Rasterize opaque glyph footprints using the same paint traversal as
        // the GUI. This checks compositing, independently of OS font shaping.
        for (columns, input, expected) in [
            (12, "A한글中🙂B", vec!["A한한글글中中🙂🙂B__"]),
            (6, "ABCDE한B", vec!["ABCDE_", "한한B___"]),
            (6, "\x1b[44m한\x1b[41m글\x1b[0m!", vec!["한한글글!_"]),
            (6, "한\x1b[8m글\x1b[0m!", vec!["한한__!_"]),
        ] {
            let rows = expected.len();
            let mut term = Term::new(
                Config::default(),
                &TermSize::new(columns, rows),
                VoidListener,
            );
            let mut parser: Processor = Processor::new();
            parser.advance(&mut term, input.as_bytes());
            let mut pixels = vec![vec!['?'; columns]; rows];
            paint_grid(&term, |layer, indexed| {
                let row = indexed.point.line.0 as usize;
                let column = indexed.point.column.0;
                match layer {
                    CellLayer::Background => pixels[row][column] = '_',
                    CellLayer::Glyph => {
                        let width = if indexed.cell.flags.contains(Flags::WIDE_CHAR) {
                            2
                        } else {
                            1
                        };
                        pixels[row][column..(column + width).min(columns)].fill(indexed.cell.c);
                    }
                }
            });
            let actual: Vec<String> = pixels
                .into_iter()
                .map(|row| row.into_iter().collect())
                .collect();
            assert_eq!(actual, expected, "input: {input:?}");
        }
    }
}
