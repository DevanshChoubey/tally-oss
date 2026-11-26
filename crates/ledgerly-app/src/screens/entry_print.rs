//! Printing the voucher an entry screen just saved (round s18).
//!
//! The accounting product prints a recorded voucher with Ctrl+P: "Record your invoice/voucher
//! with all the required transaction details, press Ctrl+P (Current), select the printer, and
//! print" (help.tallysolutions.com/print-documents/, read 2026-11-26); the flow is print, preview,
//! then print (help.tallysolutions.com/print-invoices-reports/, read 2026-11-25). The same
//! page lists Alt+P as the Print menu (printer set-up), so Alt+P is not bound to printing.
//!
//! An entry screen starts a fresh voucher after every save, and Ledgerly has no screen that alters
//! a saved voucher, so the voucher Ctrl+P prints is the one this screen saved last. Lines typed
//! but not saved are not printed (the pages do not say an unrecorded voucher can be), and a screen
//! that has saved nothing says so. The pages are the print engine's own, as for every report: see
//! [`crate::screens::print_job`]. Both entry screens (`voucher_entry` and `item_invoice`) use this
//! one type, so their words live here and nowhere else.

use egui::{Key, Modifiers, Ui};
use ledgerly_core::VoucherId;
use ledgerly_model::{Masters, Voucher};

use crate::AppCtx;
use crate::screens::print_job::PrintJob;
use crate::screens::print_preview;
use crate::screens::report_view::Notice;
use crate::ui::page_head::page_head;

/// Said when Ctrl+P is pressed on a screen that has saved no voucher and holds no lines.
fn nothing_saved() -> String {
    crate::i18n::t("entry_print.nothing_saved")
}
/// Said when Ctrl+P is pressed with lines typed and not saved.
fn save_first() -> String {
    crate::i18n::t("entry_print.save_first")
}
/// Said when Ctrl+P is pressed and no company is open.
fn no_company() -> String {
    crate::i18n::t("entry_print.no_company")
}

/// The voucher this screen saved last.
#[derive(Debug, Clone)]
struct Saved {
    id: VoucherId,
    voucher: Voucher,
}

/// The head of the preview: "Journal JV/0033" and "Saved · 20 Nov 2026".
#[derive(Debug, Clone, Default)]
struct Head {
    title: String,
    subtitle: String,
}

/// What Ctrl+P did on an entry screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrintKey {
    /// The key was not pressed, or a preview is open and handles it.
    None,
    /// The print started: the preview opens when the engine has laid the pages out.
    Started,
    /// Nothing was printed; the screen shows this sentence.
    Said(String),
}

/// The print state of one entry screen.
#[derive(Debug, Default)]
pub struct EntryPrint {
    print: PrintJob,
    saved: Option<Saved>,
    head: Head,
}

impl EntryPrint {
    /// Remembers the voucher the engine posted, so Ctrl+P can print it. `None` (a post that
    /// reported no voucher) forgets the earlier one: it must never print a voucher other than the
    /// last one saved.
    pub fn remember(&mut self, posted: Option<Voucher>) {
        self.saved = posted.map(|voucher| Saved {
            id: voucher.id,
            voucher,
        });
    }

    /// The pages the preview shows, when it is open.
    #[must_use]
    pub fn preview(&self) -> Option<&ledgerly_api::PrintedDocument> {
        self.print.preview()
    }

    /// The preview's head, "Journal JV/0033", while it is open.
    #[must_use]
    pub fn page_title(&self) -> Option<&str> {
        self.preview().map(|_| self.head.title.as_str())
    }

    /// The preview's subtitle, "Saved · 20 Nov 2026", while it is open.
    #[must_use]
    pub fn subtitle(&self) -> Option<&str> {
        self.preview().map(|_| self.head.subtitle.as_str())
    }

    /// Replaces the save dialog the preview opens (tests).
    pub fn set_picker(&mut self, picker: crate::screens::print_job::SavePicker) {
        self.print.set_picker(picker);
    }

    /// Reads Ctrl+P. `has_lines` says the screen holds lines that are not saved. `title` is the
    /// screen's own name for the voucher ("Sales invoice", "Journal").
    pub fn on_ctrl_p(&mut self, ui: &Ui, ctx: &AppCtx, title: &str, has_lines: bool) -> PrintKey {
        if self.preview().is_some()
            || !ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::P))
        {
            return PrintKey::None;
        }
        let Some(worker) = ctx.api.as_ref() else {
            return PrintKey::Said(no_company());
        };
        if has_lines {
            return PrintKey::Said(save_first());
        }
        let Some(saved) = self.saved.as_ref() else {
            return PrintKey::Said(nothing_saved());
        };
        let masters = ctx.books.as_ref().map(|b| &b.masters);
        self.head = head_for(&saved.voucher, title, masters);
        let id = saved.id;
        self.print
            .start(worker, &self.head.title.clone(), move |api, setup| {
                api.print_voucher_document_with(id, setup)
            });
        PrintKey::Started
    }

    /// Moves the print on one frame. Returns the notice to show when the engine refused the
    /// voucher or the PDF was written.
    pub fn tick(&mut self, ctx: &AppCtx) -> Option<Notice> {
        let note = self.print.tick();
        self.print.run_update(ctx.api.as_ref());
        note
    }

    /// Draws the preview in place of the screen: the head, then the engine's pages and the card.
    /// `false` when no preview is open and the screen draws itself.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        if self.print.preview().is_none() {
            if self.print.is_rendering() {
                print_preview::draw_busy(ui);
                return true;
            }
            return false;
        }
        page_head(ui, &self.head.title, Some(&self.head.subtitle), |_| {});
        self.print.draw_preview(ui);
        ui.ctx().request_repaint();
        true
    }
}

/// The preview's head for `voucher`: the screen's name and the number, then what was saved. The
/// party is named when the voucher has one; a voucher with no number is not given one here.
fn head_for(voucher: &Voucher, title: &str, masters: Option<&Masters>) -> Head {
    let title = match voucher.number.as_deref() {
        Some(number) => format!("{title} {number}"),
        None => title.to_owned(),
    };
    let party = voucher
        .party
        .and_then(|id| masters.and_then(|m| m.ledgers.get(&id)))
        .map(|l| l.name.clone());
    let mut words = vec![crate::i18n::t("entry_print.saved")];
    words.extend(party);
    words.push(crate::ui::format::date(voucher.date));
    Head {
        title,
        subtitle: words.join(" \u{b7} "),
    }
}
