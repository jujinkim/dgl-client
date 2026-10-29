use alacritty_terminal::{
    Term,
    grid::Indexed,
    term::cell::{Cell, Flags},
};

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
