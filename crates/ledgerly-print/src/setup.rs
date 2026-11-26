//! What the user can set before a document prints: paper size, number of copies, the pages to
//! print and the number the first page starts from.
//!
//! Source: the product's help page "Configure for Print, Export, Share"
//! (<https://help.tallysolutions.com/configure-for-print-export-share/>, read 2026-11-26) lists
//! "Number of copies", and under "Pages to Print" (F10) "Number to start from first page" ("sets
//! the starting page number, which then increments") and "Page Range" ("prints only the pages you
//! specify"). The page "Print Customisation FAQ"
//! (<https://help.tallysolutions.com/print-customisation-faq/>, read 2026-11-26) names A5 as the
//! paper that prints an invoice on half an A4 sheet. The pages say nothing about the syntax of a
//! page range, so the one accepted here is the usual one: numbers and `from-to` ranges separated
//! by commas. Margins are documented only for invoice templates and are not offered.

use crate::template::PrintError;

/// The most copies one print makes (the help pages give no limit; this keeps a file bounded).
pub const MAX_COPIES: u8 = 99;

/// A paper size the printed pages can be laid out on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    /// 210 mm by 297 mm.
    A4,
    /// 148 mm by 210 mm.
    A5,
}

impl Paper {
    /// Width and height in millimetres, portrait.
    #[must_use]
    pub const fn millimetres(self) -> (u16, u16) {
        match self {
            Self::A4 => (210, 297),
            Self::A5 => (148, 210),
        }
    }
}

/// Which pages of the document print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageSelection {
    /// Every page.
    All,
    /// The pages in these inclusive `(from, to)` ranges, by the number printed on each page.
    Ranges(Vec<(u32, u32)>),
}

impl PageSelection {
    /// Reads a page range as typed: `3`, `2-4` or `1, 3-5`. Blank means every page.
    ///
    /// # Errors
    /// [`PrintError::Setup`] naming the part that is not a page number or range, a page number
    /// of 0, or a range that runs backwards.
    pub fn parse(text: &str) -> Result<Self, PrintError> {
        if text.trim().is_empty() {
            return Ok(Self::All);
        }
        let mut ranges = Vec::new();
        for part in text.split(',') {
            let part = part.trim();
            let (from, to) = if let Some((a, b)) = part.split_once('-') {
                (page_number(a, part)?, page_number(b, part)?)
            } else {
                let n = page_number(part, part)?;
                (n, n)
            };
            if from > to {
                return Err(setup_error(format!(
                    "the page range \"{part}\" runs backwards; write it as {to}-{from}"
                )));
            }
            ranges.push((from, to));
        }
        Ok(Self::Ranges(ranges))
    }

    /// The selection as typed: `all`, or the ranges such as `1, 3-5`.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::All => "all".to_owned(),
            Self::Ranges(ranges) => ranges
                .iter()
                .map(|(a, b)| {
                    if a == b {
                        a.to_string()
                    } else {
                        format!("{a}-{b}")
                    }
                })
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    /// Whether page number `n` is selected.
    #[must_use]
    pub fn contains(&self, n: u32) -> bool {
        match self {
            Self::All => true,
            Self::Ranges(ranges) => ranges.iter().any(|(a, b)| (*a..=*b).contains(&n)),
        }
    }
}

fn page_number(text: &str, whole: &str) -> Result<u32, PrintError> {
    match text.trim().parse::<u32>() {
        Ok(0) => Err(setup_error(format!("\"{whole}\": page numbers start at 1"))),
        Ok(n) => Ok(n),
        Err(_) => Err(setup_error(format!(
            "\"{whole}\" is not a page number or a range such as 2-4"
        ))),
    }
}

pub(crate) fn setup_error(reason: String) -> PrintError {
    PrintError::Setup { reason }
}

/// The settings of one print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintSetup {
    /// The paper; `None` keeps the document's own (A4, or the template's page).
    pub paper: Option<Paper>,
    /// Copies of the document in the file, 1 to [`MAX_COPIES`].
    pub copies: u8,
    /// The pages that print.
    pub pages: PageSelection,
    /// The number printed on the first page; the next pages count up from it.
    pub first_page_number: u32,
}

impl Default for PrintSetup {
    fn default() -> Self {
        Self {
            paper: None,
            copies: 1,
            pages: PageSelection::All,
            first_page_number: 1,
        }
    }
}

impl PrintSetup {
    /// Checks the settings that do not depend on the document.
    ///
    /// # Errors
    /// [`PrintError::Setup`] for 0 copies, more than [`MAX_COPIES`], or a first page number of 0.
    pub fn check(&self) -> Result<(), PrintError> {
        if self.copies == 0 || self.copies > MAX_COPIES {
            return Err(setup_error(format!(
                "the number of copies must be from 1 to {MAX_COPIES}, not {}",
                self.copies
            )));
        }
        if self.first_page_number == 0 {
            return Err(setup_error("page numbers start at 1".to_owned()));
        }
        Ok(())
    }
}
