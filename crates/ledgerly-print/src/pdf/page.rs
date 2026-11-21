//! Page layout and PDF writing for the invoice renderer (card PR2).
//!
//! All layout is whole points in `i32`, measured from the top-left of the page. Floats appear
//! only at the `pdf-writer` boundary, converted from integers with `f32::from` (rule A3: no float
//! arithmetic). Text uses the built-in Helvetica and Helvetica-Bold fonts with `WinAnsiEncoding`;
//! only plain ASCII (code points 32 to 126) can be printed. Any other character is reported as
//! [`PrintError::Unsupported`] instead of being replaced (rule A1).

use pdf_writer::{Content, Name, Pdf, Rect, Ref, Str};

use crate::template::PrintError;

/// Page margin in points.
pub(super) const MARGIN: i32 = 36;
/// Space kept free at the bottom of every page for the page number.
const FOOTER: i32 = 28;
/// Padding inside table cells.
const PAD: i32 = 3;
/// Body text size.
pub(super) const BODY: i32 = 10;
/// Table text size.
const TABLE: i32 = 9;

/// Advance widths of Helvetica for code points 32 to 126, in 1000ths of the font size.
///
/// Provenance: recalled from the published Helvetica AFM metrics, not re-checked. They only
/// affect where text is aligned and wrapped, never which characters are printed.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
    278, // space to /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // 0 to 9
    278, 278, 584, 584, 584, 556, 1015, // : to @
    667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, // A to M
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, // N to Z
    278, 278, 278, 469, 556, 333, // [ to `
    556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, // a to m
    556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, // n to z
    334, 260, 334, 584, // { to ~
];

/// Width used for a character outside the table (layout only; such text is refused earlier).
const FALLBACK_WIDTH: u16 = 556;

/// Converts a layout value to the `f32` the PDF writer takes, clamped to the `i16` range.
fn pt(v: i32) -> f32 {
    let clamped = v.clamp(i32::from(i16::MIN), i32::from(i16::MAX));
    f32::from(i16::try_from(clamped).unwrap_or(0))
}

fn to_i32(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// Width of `text` in points at `size`. Bold text is estimated 10 percent wider.
pub(super) fn text_width(text: &str, size: i32, bold: bool) -> i32 {
    let mut units: i32 = 0;
    for ch in text.chars() {
        let width = u8::try_from(ch)
            .ok()
            .and_then(|b| b.checked_sub(32))
            .and_then(|i| HELVETICA.get(usize::from(i)).copied())
            .unwrap_or(FALLBACK_WIDTH);
        units = units.saturating_add(i32::from(width));
    }
    if bold {
        units = units.saturating_add(units / 10);
    }
    units.saturating_mul(size) / 1000
}

/// Splits `text` into lines no wider than `max_width`, breaking at spaces and, for a word wider
/// than the line, between characters. Explicit line breaks in `text` are kept.
pub(super) fn wrap(text: &str, max_width: i32, size: i32, bold: bool) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        wrap_paragraph(paragraph, max_width, size, bold, &mut lines);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn wrap_paragraph(text: &str, max_width: i32, size: i32, bold: bool, lines: &mut Vec<String>) {
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if text_width(&candidate, size, bold) <= max_width {
            current = candidate;
            continue;
        }
        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
        if text_width(word, size, bold) <= max_width {
            current = word.to_owned();
            continue;
        }
        for ch in word.chars() {
            let mut next = current.clone();
            next.push(ch);
            if text_width(&next, size, bold) > max_width && !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current.push(ch);
            } else {
                current = next;
            }
        }
    }
    lines.push(current);
}

/// Line height for text of `size`.
pub(super) fn leading(size: i32) -> i32 {
    size.saturating_add(size / 4).saturating_add(1)
}

/// One drawing operation on a page. Positions are in points from the top-left.
enum Op {
    Text {
        x: i32,
        baseline: i32,
        size: i32,
        bold: bool,
        text: String,
    },
    Rule {
        x1: i32,
        x2: i32,
        y: i32,
    },
    Fill {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
}

/// How a table column aligns its text.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Align {
    Left,
    Right,
}

/// A table column: heading, share of the width and alignment.
pub(super) struct Column {
    pub(super) title: &'static str,
    pub(super) weight: i32,
    pub(super) align: Align,
}

fn refuse_char(ch: char) -> PrintError {
    PrintError::Unsupported {
        reason: format!(
            "the character {ch:?} cannot be printed: the built-in font covers plain ASCII only"
        ),
    }
}

fn push_text(
    ops: &mut Vec<Op>,
    x: i32,
    baseline: i32,
    size: i32,
    bold: bool,
    text: &str,
) -> Result<(), PrintError> {
    if let Some(bad) = text.chars().find(|c| !matches!(c, ' '..='~')) {
        return Err(refuse_char(bad));
    }
    ops.push(Op::Text {
        x,
        baseline,
        size,
        bold,
        text: text.to_owned(),
    });
    Ok(())
}

/// The page being laid out, top to bottom, with a new page started when one fills up.
pub(super) struct Canvas {
    width: i32,
    height: i32,
    done: Vec<Vec<Op>>,
    current: Vec<Op>,
    y: i32,
}

impl Canvas {
    /// A canvas of one empty page, `width` by `height` points.
    pub(super) fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            done: Vec::new(),
            current: Vec::new(),
            y: MARGIN,
        }
    }

    /// Width available between the margins.
    pub(super) fn content_width(&self) -> i32 {
        self.width.saturating_sub(MARGIN.saturating_mul(2))
    }

    /// X of the right margin.
    pub(super) fn right(&self) -> i32 {
        self.width.saturating_sub(MARGIN)
    }

    fn bottom(&self) -> i32 {
        self.height.saturating_sub(MARGIN).saturating_sub(FOOTER)
    }

    /// Moves down `n` points.
    pub(super) fn skip(&mut self, n: i32) {
        self.y = self.y.saturating_add(n);
    }

    /// Starts a new page when `h` more points do not fit; returns whether it did.
    pub(super) fn ensure(&mut self, h: i32) -> bool {
        if self.y.saturating_add(h) > self.bottom() && self.y > MARGIN {
            self.done.push(std::mem::take(&mut self.current));
            self.y = MARGIN;
            true
        } else {
            false
        }
    }

    /// Prints `text` at `x` on the current line (the line is `size` tall, not yet advanced).
    pub(super) fn text_at(
        &mut self,
        x: i32,
        size: i32,
        bold: bool,
        text: &str,
    ) -> Result<(), PrintError> {
        let baseline = self.y.saturating_add(size);
        push_text(&mut self.current, x, baseline, size, bold, text)
    }

    /// Prints wrapped text from the left margin, one line after another.
    pub(super) fn paragraph(
        &mut self,
        text: &str,
        size: i32,
        bold: bool,
    ) -> Result<(), PrintError> {
        for line in wrap(text, self.content_width(), size, bold) {
            self.ensure(leading(size));
            self.text_at(MARGIN, size, bold, &line)?;
            self.skip(leading(size));
        }
        Ok(())
    }

    /// Prints `label` at the left margin and `value` against the right margin on one line.
    pub(super) fn label_value(
        &mut self,
        label: &str,
        value: &str,
        size: i32,
        bold: bool,
    ) -> Result<(), PrintError> {
        self.ensure(leading(size));
        self.text_at(MARGIN, size, bold, label)?;
        let x = self
            .right()
            .saturating_sub(text_width(value, size, bold))
            .max(MARGIN);
        self.text_at(x, size, bold, value)?;
        self.skip(leading(size));
        Ok(())
    }

    fn rule(&mut self) {
        let (x1, x2, y) = (MARGIN, self.right(), self.y);
        self.current.push(Op::Rule { x1, x2, y });
    }

    /// Draws a square-module code with its top-left at the current line.
    ///
    /// `modules` holds `side * side` entries, row by row, `true` for a dark module.
    pub(super) fn modules(
        &mut self,
        modules: &[bool],
        side: usize,
        module: i32,
    ) -> Result<(), PrintError> {
        if side == 0 || modules.len() != side.saturating_mul(side) {
            return Err(PrintError::Render {
                reason: "the QR code is not square".to_owned(),
            });
        }
        let total = to_i32(side).saturating_mul(module);
        self.ensure(total);
        let mut y = self.y;
        for row in modules.chunks(side) {
            let mut x = MARGIN;
            for dark in row {
                if *dark {
                    self.current.push(Op::Fill {
                        x,
                        y,
                        w: module,
                        h: module,
                    });
                }
                x = x.saturating_add(module);
            }
            y = y.saturating_add(module);
        }
        self.y = y;
        Ok(())
    }

    fn column_widths(&self, cols: &[Column]) -> Vec<i32> {
        let total: i32 = cols
            .iter()
            .fold(0_i32, |a, c| a.saturating_add(c.weight.max(1)));
        let width = self.content_width();
        let mut used = 0_i32;
        let mut widths = Vec::with_capacity(cols.len());
        for (n, col) in cols.iter().enumerate() {
            let w = if n.saturating_add(1) == cols.len() {
                width.saturating_sub(used)
            } else {
                width.saturating_mul(col.weight.max(1)) / total.max(1)
            };
            used = used.saturating_add(w);
            widths.push(w);
        }
        widths
    }

    /// Prints a table: a heading row and then `rows`. A row that does not fit starts a new page,
    /// which repeats the heading. Every row must have one cell per column.
    pub(super) fn table(
        &mut self,
        cols: &[Column],
        rows: &[Vec<String>],
    ) -> Result<(), PrintError> {
        let widths = self.column_widths(cols);
        let heading: Vec<String> = cols.iter().map(|c| c.title.to_owned()).collect();
        self.table_row(cols, &widths, &heading, true)?;
        for row in rows {
            if row.len() != cols.len() {
                return Err(PrintError::Render {
                    reason: "a table row has a different number of cells than columns".to_owned(),
                });
            }
            self.table_row(cols, &widths, row, false)?;
        }
        Ok(())
    }

    fn table_row(
        &mut self,
        cols: &[Column],
        widths: &[i32],
        cells: &[String],
        heading: bool,
    ) -> Result<(), PrintError> {
        let wrapped: Vec<Vec<String>> = cells
            .iter()
            .zip(widths)
            .map(|(t, w)| wrap(t, w.saturating_sub(PAD.saturating_mul(2)), TABLE, heading))
            .collect();
        let lines = wrapped.iter().map(Vec::len).max().unwrap_or(1);
        let height = to_i32(lines)
            .saturating_mul(leading(TABLE))
            .saturating_add(PAD.saturating_mul(2));
        if !heading && self.ensure(height.saturating_add(leading(TABLE))) {
            let titles: Vec<String> = cols.iter().map(|c| c.title.to_owned()).collect();
            self.table_row(cols, widths, &titles, true)?;
        }
        let mut x0 = MARGIN;
        for ((col, width), lines) in cols.iter().zip(widths).zip(&wrapped) {
            let mut baseline = self.y.saturating_add(PAD).saturating_add(TABLE);
            for line in lines {
                let x = match col.align {
                    Align::Left => x0.saturating_add(PAD),
                    Align::Right => x0
                        .saturating_add(*width)
                        .saturating_sub(PAD)
                        .saturating_sub(text_width(line, TABLE, heading)),
                };
                push_text(&mut self.current, x, baseline, TABLE, heading, line)?;
                baseline = baseline.saturating_add(leading(TABLE));
            }
            x0 = x0.saturating_add(*width);
        }
        self.skip(height);
        self.rule();
        Ok(())
    }

    /// Adds the page numbers and writes the document.
    pub(super) fn finish(mut self) -> Result<Vec<u8>, PrintError> {
        self.done.push(std::mem::take(&mut self.current));
        let total = self.done.len();
        let baseline = self.height.saturating_sub(MARGIN);
        let centre = self.width / 2;
        let mut number = 0_usize;
        for ops in &mut self.done {
            number = number.saturating_add(1);
            let label = format!("Page {number} of {total}");
            let x = centre.saturating_sub(text_width(&label, TABLE, false) / 2);
            push_text(ops, x, baseline, TABLE, false, &label)?;
        }
        write_pdf(self.width, self.height, &self.done)
    }
}

fn too_large() -> PrintError {
    PrintError::Render {
        reason: "the document has too many pages to number".to_owned(),
    }
}

/// Object numbers: 1 catalog, 2 page tree, 3 and 4 fonts, then a page and its content per page.
fn page_refs(index: usize) -> Result<(Ref, Ref), PrintError> {
    let page = index
        .checked_mul(2)
        .and_then(|n| n.checked_add(5))
        .and_then(|n| i32::try_from(n).ok())
        .filter(|n| *n > 0 && *n < i32::MAX)
        .ok_or_else(too_large)?;
    Ok((Ref::new(page), Ref::new(page + 1)))
}

fn content_stream(height: i32, ops: &[Op]) -> Vec<u8> {
    let mut c = Content::new();
    for op in ops {
        match op {
            Op::Text {
                x,
                baseline,
                size,
                bold,
                text,
            } => {
                let font: &[u8] = if *bold { b"F2" } else { b"F1" };
                c.begin_text();
                c.set_font(Name(font), pt(*size));
                c.next_line(pt(*x), pt(height.saturating_sub(*baseline)));
                c.show(Str(text.as_bytes()));
                c.end_text();
            }
            Op::Rule { x1, x2, y } => {
                let y = pt(height.saturating_sub(*y));
                c.set_line_width(0.5);
                c.move_to(pt(*x1), y);
                c.line_to(pt(*x2), y);
                c.stroke();
            }
            Op::Fill { x, y, w, h } => {
                c.rect(
                    pt(*x),
                    pt(height.saturating_sub(y.saturating_add(*h))),
                    pt(*w),
                    pt(*h),
                );
                c.fill_nonzero();
            }
        }
    }
    let buf = c.finish();
    buf.to_vec()
}

fn write_pdf(width: i32, height: i32, pages: &[Vec<Op>]) -> Result<Vec<u8>, PrintError> {
    let catalog = Ref::new(1);
    let tree = Ref::new(2);
    let regular = Ref::new(3);
    let bold = Ref::new(4);
    let mut pdf = Pdf::new();
    pdf.catalog(catalog).pages(tree);
    pdf.type1_font(regular)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    pdf.type1_font(bold)
        .base_font(Name(b"Helvetica-Bold"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    let mut kids = Vec::with_capacity(pages.len());
    for (index, ops) in pages.iter().enumerate() {
        let (page_ref, content_ref) = page_refs(index)?;
        kids.push(page_ref);
        {
            let mut page = pdf.page(page_ref);
            page.parent(tree)
                .media_box(Rect::new(0.0, 0.0, pt(width), pt(height)))
                .contents(content_ref);
            page.resources()
                .fonts()
                .pair(Name(b"F1"), regular)
                .pair(Name(b"F2"), bold);
        }
        let data = content_stream(height, ops);
        pdf.stream(content_ref, &data);
    }
    let count = i32::try_from(kids.len()).map_err(|_| too_large())?;
    pdf.pages(tree).kids(kids).count(count);
    Ok(pdf.finish())
}
