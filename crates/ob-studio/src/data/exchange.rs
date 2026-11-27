//! Moving whole blocks of cells in and out of the Data grid: a selection copied as tab-separated text, text pasted
//! into a selection, one value filled into several cells, and the visible table written to and read back from a
//! CSV file. Every cell that goes in is checked by the same rules as a single edit ([`super::op_for`]), and a block
//! is all or none: one refused cell refuses the block, and the words say which cell and why. A block that is made is
//! one undo step.

use std::collections::HashMap;

use glam::Vec3;
use ob_map::tools::map::ops::{Change, Op};

use super::{Grid, Ty, op_for_in, parse_vec3};
use crate::session::MapSession;

/// A block of cells of what the grid shows: rows `r0..=r1` of the shown rows, columns `c0..=c1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub r0: usize,
    pub c0: usize,
    pub r1: usize,
    pub c1: usize,
}

impl Span {
    /// The block between two corners, in either order.
    pub fn between(a: (usize, usize), b: (usize, usize)) -> Span {
        Span { r0: a.0.min(b.0), c0: a.1.min(b.1), r1: a.0.max(b.0), c1: a.1.max(b.1) }
    }

    pub fn rows(&self) -> usize {
        self.r1 - self.r0 + 1
    }

    pub fn cols(&self) -> usize {
        self.c1 - self.c0 + 1
    }

    pub fn cells(&self) -> usize {
        self.rows() * self.cols()
    }

    pub fn contains(&self, row: usize, col: usize) -> bool {
        (self.r0..=self.r1).contains(&row) && (self.c0..=self.c1).contains(&col)
    }
}

/// One cell to write: the row's key, the column's field and the text as typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellEdit {
    pub key: String,
    pub field: &'static str,
    pub text: String,
}

/// A cell the block cannot write, and why (the words are the cell's: "Laps of row x: 99 is out of range").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub row: String,
    pub column: String,
    pub why: String,
}

impl Refusal {
    fn sentence(&self) -> String {
        format!("{} of row {}: {}", self.column, self.row, self.why)
    }
}

/// The refusals as one line: the first three, and how many more.
pub fn refusals_sentence(r: &[Refusal]) -> String {
    let mut s = r.iter().take(3).map(Refusal::sentence).collect::<Vec<_>>().join("; ");
    if r.len() > 3 {
        s.push_str(&format!(" (and {} more)", r.len() - 3));
    }
    s
}

/// The ops of `edits` on the mod `map`, every cell checked: all of them, or every refusal. A heading and a position
/// of one row go together (the heading turns the prop where the position puts it).
pub fn plan(map: &ob_map::tools::map::format::MapMod, pack: &str, kind: &str, edits: &[CellEdit]) -> Result<Vec<Op>, Vec<Refusal>> {
    let cols = super::columns(kind);
    let label = |f: &str| cols.iter().find(|c| c.field == f).map_or(f.to_string(), |c| c.label.to_string());
    let moved: HashMap<&str, Vec3> = edits
        .iter()
        .filter(|e| e.field == "position")
        .filter_map(|e| parse_vec3(&e.text).ok().map(|v| (e.key.as_str(), v)))
        .collect();
    let mut ordered: Vec<&CellEdit> = edits.iter().collect();
    ordered.sort_by_key(|e| e.field == "heading");
    let (mut ops, mut bad) = (Vec::new(), Vec::new());
    for e in ordered {
        match op_for_in(map, pack, kind, &e.key, e.field, &e.text, moved.get(e.key.as_str()).copied()) {
            Ok(op) => ops.push(op),
            Err(why) => bad.push(Refusal { row: e.key.clone(), column: label(e.field), why }),
        }
    }
    if bad.is_empty() { Ok(ops) } else { Err(bad) }
}

/// `edits` written as one undo step named `label`, all or none.
pub fn edit_cells(
    session: &mut MapSession,
    pack: &str,
    kind: &str,
    edits: &[CellEdit],
    label: &str,
) -> Result<Change, String> {
    let ops = plan(&session.map, pack, kind, edits).map_err(|r| refusals_sentence(&r))?;
    session
        .run(Op::Group { label: label.to_string(), ops })
        .map_err(|e| session.refused_why.take().unwrap_or_else(|| e.to_string()))
}

// ---- CSV -----------------------------------------------------------------------------------------------------------

/// Rows of fields from CSV text (quotes, doubled quotes, commas and line breaks inside quotes; `\n` or `\r\n`).
/// `sep` is the field separator. A blank line is left out; an unterminated quote is an error.
pub fn csv_read(text: &str, sep: char) -> Result<Vec<Vec<String>>, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let (mut rows, mut row, mut field) = (Vec::new(), Vec::new(), String::new());
    let (mut quoted, mut was_quoted, mut any) = (false, false, false);
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        any = true;
        if quoted {
            if c == '"' {
                if it.peek() == Some(&'"') {
                    it.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' && field.is_empty() && !was_quoted {
            quoted = true;
            was_quoted = true;
        } else if c == sep {
            row.push(std::mem::take(&mut field));
            was_quoted = false;
        } else if c == '\n' || c == '\r' {
            if c == '\r' && it.peek() == Some(&'\n') {
                it.next();
            }
            if !(row.is_empty() && field.is_empty() && !was_quoted) {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            was_quoted = false;
        } else {
            field.push(c);
        }
    }
    if quoted {
        return Err("a quote is never closed".into());
    }
    if any && !(row.is_empty() && field.is_empty() && !was_quoted) {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

/// Rows of fields as CSV text: a field with a comma, a quote or a line break is quoted; lines end in `\n`.
pub fn csv_write(rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    for r in rows {
        for (i, f) in r.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            if f.contains([',', '"', '\n', '\r']) {
                out.push('"');
                out.push_str(&f.replace('"', "\"\""));
                out.push('"');
            } else {
                out.push_str(f);
            }
        }
        out.push('\n');
    }
    out
}

/// The header word of the key column.
const ROW: &str = "row";

impl Grid {
    /// The visible table as CSV: a `row` column with each row's key, then the columns by their field names, in the
    /// shown order of the shown rows.
    pub fn csv(&self) -> String {
        let mut rows = vec![std::iter::once(ROW.to_string()).chain(self.columns.iter().map(|c| c.field.to_string())).collect::<Vec<_>>()];
        for i in 0..self.view.len() {
            let mut r = vec![self.key(i).unwrap_or_default().to_string()];
            r.extend((0..self.columns.len()).map(|c| self.text_at(self.view[i], c)));
            rows.push(r);
        }
        csv_write(&rows)
    }

    /// The block as text for the clipboard: tabs between cells, a line per row.
    pub fn copy_text(&self, span: Span) -> String {
        (span.r0..=span.r1.min(self.view.len().saturating_sub(1)))
            .map(|i| {
                (span.c0..=span.c1.min(self.columns.len().saturating_sub(1)))
                    .map(|c| self.text_at(self.view[i], c).replace(['\t', '\n', '\r'], " "))
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `text` for the cell of row `row` (index into all rows), column `col`: None when it already reads so.
    fn cell_edit(&self, row: usize, col: usize, text: &str) -> Result<Option<CellEdit>, Refusal> {
        let c = &self.columns[col];
        let key = self.rows[row].key.clone();
        if self.text_at(row, col) == text {
            return Ok(None);
        }
        if !c.editable {
            return Err(Refusal { row: key, column: c.label.to_string(), why: "cannot be changed here yet".into() });
        }
        Ok(Some(CellEdit { key, field: c.field, text: text.to_string() }))
    }

    /// One value written in every cell of the block (the cells that already read so are left out).
    pub fn fill(&self, span: Span, text: &str) -> Result<Vec<CellEdit>, Vec<Refusal>> {
        let cells = (span.r0..=span.r1).flat_map(|r| (span.c0..=span.c1).map(move |c| (r, c)));
        self.collect(cells.filter_map(|(r, c)| Some((*self.view.get(r)?, c, text.to_string()))))
    }

    fn collect(&self, cells: impl Iterator<Item = (usize, usize, String)>) -> Result<Vec<CellEdit>, Vec<Refusal>> {
        let (mut edits, mut bad) = (Vec::new(), Vec::new());
        for (row, col, text) in cells {
            match self.cell_edit(row, col, &text) {
                Ok(Some(e)) => edits.push(e),
                Ok(None) => {}
                Err(r) => bad.push(r),
            }
        }
        if bad.is_empty() { Ok(edits) } else { Err(bad) }
    }

    /// Pasted text as edits. Tabs separate cells and lines rows; text with no tab is CSV (a position's three numbers
    /// stay one cell). One value pastes into every cell of the selection; a block starts at the selection's first
    /// cell and has to fit the table.
    pub fn paste(&self, span: Span, text: &str) -> Result<Vec<CellEdit>, Vec<Refusal>> {
        let whole = |why: String| Err(vec![Refusal { row: String::new(), column: "Paste".into(), why }]);
        let block = if text.contains('\t') {
            csv_read(text, '\t')
        } else {
            csv_read(text, ',').map(|rows| {
                let vec3 = self.columns.get(span.c0).is_some_and(|c| c.ty == Ty::Vec3);
                rows.into_iter().map(|r| if vec3 && r.len() == 3 { vec![r.iter().map(|f| f.trim()).collect::<Vec<_>>().join(", ")] } else { r }).collect()
            })
        };
        let block = match block {
            Ok(b) if !b.is_empty() => b,
            Ok(_) => return whole("there is nothing to paste".into()),
            Err(e) => return whole(e),
        };
        let (h, w) = (block.len(), block.iter().map(Vec::len).max().unwrap_or(0));
        if h == 1 && w == 1 {
            return self.fill(span, block[0][0].trim_end_matches(['\r', '\n']));
        }
        let (rows, cols) = (self.view.len(), self.columns.len());
        if span.r0 + h > rows || span.c0 + w > cols {
            return whole(format!(
                "{h} rows of {w} cells do not fit here: there is room for {} rows of {} cells",
                rows.saturating_sub(span.r0),
                cols.saturating_sub(span.c0)
            ));
        }
        self.collect(block.into_iter().enumerate().flat_map(|(r, line)| {
            line.into_iter().enumerate().map(move |(c, t)| (span.r0 + r, span.c0 + c, t))
        }).map(|(r, c, t)| (self.view[r], c, t)))
    }

    /// A CSV table (as [`csv`](Self::csv) writes it) as edits: only the cells that differ from the grid's now, each
    /// by its row's key and column name (the field or its label, any case). Unknown rows and columns are refused.
    pub fn import(&self, text: &str) -> Result<Vec<CellEdit>, Vec<Refusal>> {
        let whole = |why: String| Err(vec![Refusal { row: String::new(), column: "Import".into(), why }]);
        let rows = match csv_read(text, ',') {
            Ok(r) if r.len() >= 2 => r,
            Ok(_) => return whole("the file has no header and rows".into()),
            Err(e) => return whole(e),
        };
        let mut cols: Vec<Option<usize>> = Vec::new();
        let mut bad = Vec::new();
        if !rows[0].first().is_some_and(|h| h.trim().eq_ignore_ascii_case(ROW)) {
            return whole(format!("the first column has to be {ROW}, the row's key"));
        }
        for h in &rows[0][1..] {
            let h = h.trim();
            let at = self.columns.iter().position(|c| c.field.eq_ignore_ascii_case(h) || c.label.eq_ignore_ascii_case(h));
            if at.is_none() {
                bad.push(Refusal { row: String::new(), column: h.to_string(), why: "is not a column of this table".into() });
            }
            cols.push(at);
        }
        let by_key: HashMap<&str, usize> = self.rows.iter().enumerate().map(|(i, r)| (r.key.as_str(), i)).collect();
        let mut cells = Vec::new();
        for r in &rows[1..] {
            let key = r[0].trim();
            let Some(&row) = by_key.get(key) else {
                bad.push(Refusal { row: key.to_string(), column: ROW.to_string(), why: "is not a row of this table".into() });
                continue;
            };
            for (i, t) in r[1..].iter().enumerate() {
                if let Some(Some(c)) = cols.get(i) {
                    cells.push((row, *c, t.clone()));
                }
            }
        }
        match self.collect(cells.into_iter()) {
            Ok(e) if bad.is_empty() => Ok(e),
            Ok(_) => Err(bad),
            Err(mut more) => {
                bad.append(&mut more);
                Err(bad)
            }
        }
    }
}

// ---- files ---------------------------------------------------------------------------------------------------------

/// Write the grid's table to a file the player chose (never into the game's own folder).
pub fn export_file(grid: &Grid, path: &std::path::Path, game_root: Option<&std::path::Path>) -> Result<(), String> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
    crate::browser::gltf::check_target(dir, game_root).map_err(|e| e.to_string())?;
    ob_core::platform::fs::write_atomic(path, grid.csv().as_bytes(), false)
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Read a table file the player chose.
pub fn read_file(path: &std::path::Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}
