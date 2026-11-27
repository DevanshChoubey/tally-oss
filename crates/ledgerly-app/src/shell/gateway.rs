//! The home screen of an open company, and where its links lead (Wave 3 shell wiring).
//!
//! [`HomeScreen`] only records what the user asked to open (its tests pin that it returns
//! `Stay`), so this screen owns one and turns each [`HomeTarget`] into a screen:
//!
//! | Target | Opens |
//! |---|---|
//! | Sales invoice (F8) | [`navigate::sales_screen`]: an item invoice when the company keeps inventory, else an accounting invoice |
//! | Receipt (F6), Payment (F5) | voucher entry of that type |
//! | Balance sheet (B), Day book (D) | the worker-mode report |
//! | GST returns (G) | the GST screen ([`navigate::gst_screen`]) |
//! | Masters (M) | the masters screen |
//! | Palette (Ctrl+K) | the shell's palette |
//! | Unmatched bank lines | the bank reconciliation, when the company has exactly one bank ledger |
//! | Purchase from a bill (F9) | the typed purchase form, as the sidebar's Purchase row: the bill reader is not built |
//! | Describe a transaction, pending IMS | nothing yet: a notice says so |
//!
//! A target that opens nothing always leaves a notice on screen; nothing is dropped silently.
//!
//! The KPI tiles are computed by the API worker ([`HomeScreen::request_refresh`]) as of the end
//! of [`navigate::default_period`], and asked for again whenever the loaded books change.
//!
//! Which screen opens first after a company is loaded: the design mock's Home. No Tally page
//! read on 2026-11-21 states it (the TallyPrime company management and create-company pages are
//! silent), so it is `pending`.

use crate::i18n::Tr;
use ledgerly_api::ReportKind;
use ledgerly_core::{Date, FinancialYear, Outcome};
use ledgerly_model::{BaseVoucherKind, Books};

use super::navigate::{self, NavError};
use crate::screen::KeyHint;
use crate::screens::gst::{GstScreen, GstTab};
use crate::screens::home::{HomeData, HomeScreen, HomeTarget};
use crate::screens::masters::MastersScreen;
use crate::screens::review::ReviewScreen;
use crate::screens::voucher_entry::VoucherEntryScreen;
use crate::{AppCtx, Screen, ScreenAction};

/// The home screen of an open company.
pub struct GatewayScreen {
    home: HomeScreen,
    seen: Option<super::BooksPrint>,
    notice: Option<String>,
}

impl Default for GatewayScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl GatewayScreen {
    /// A home screen that fills itself from the open company's books.
    #[must_use]
    pub fn new() -> Self {
        Self {
            home: HomeScreen::new(HomeData::default()),
            seen: None,
            notice: None,
        }
    }

    /// The home screen inside, as it is drawn.
    #[must_use]
    pub fn home(&self) -> &HomeScreen {
        &self.home
    }

    /// The home screen inside, mutably (to ask for a link as a click does).
    pub fn home_mut(&mut self) -> &mut HomeScreen {
        &mut self.home
    }

    /// Why the last link opened nothing, when it did not.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Asks for the tiles again when `books` is a copy this screen has not seen.
    fn follow(&mut self, books: &Books) {
        let print = super::books_print(books);
        if self.seen == Some(print) {
            return;
        }
        self.seen = Some(print);
        match navigate::default_period(books) {
            Ok(period) => {
                let as_of = period.to();
                let mut data = self.home.data().clone();
                books.company.name.clone_into(&mut data.company_name);
                data.subtitle = format!("Figures as of {as_of}");
                self.home.set_data(data);
                self.home.request_refresh(as_of);
            }
            Err(e) => self.notice = Some(e.to_string()),
        }
    }

    /// The screen `target` opens, or `None` with a notice saying why it opens nothing.
    fn open(&mut self, target: &HomeTarget, ctx: &AppCtx) -> Option<Box<dyn Screen>> {
        let opened = match target {
            HomeTarget::SalesInvoice => {
                books(ctx).and_then(|b| navigate::sales_screen(b, navigate::today()))
            }
            HomeTarget::Receipt => books(ctx)
                .and_then(|b| entry_screen(b, BaseVoucherKind::Receipt, navigate::today())),
            HomeTarget::Payment => books(ctx)
                .and_then(|b| entry_screen(b, BaseVoucherKind::Payment, navigate::today())),
            HomeTarget::BalanceSheet { .. } => navigate::open_report(ReportKind::BalanceSheet, ctx),
            HomeTarget::DayBook => navigate::open_report(ReportKind::DayBook, ctx),
            HomeTarget::GstReturns { .. } => books(ctx)
                .and_then(|b| Ok(Box::new(gst_screen_for_target(b, target)?) as Box<dyn Screen>)),
            HomeTarget::Masters => Ok(Box::new(MastersScreen::new()) as Box<dyn Screen>),
            HomeTarget::Palette => {
                super::ask_for_palette();
                return None;
            }
            HomeTarget::UnmatchedBankLines => books(ctx).and_then(navigate::bank_entry_screen),
            // The bill reader is not built, so F9 opens the typed purchase form, as the sidebar's
            // Purchase row (F9) does.
            HomeTarget::PurchaseFromBill => books(ctx).and_then(|b| {
                navigate::invoice_screen(b, BaseVoucherKind::Purchase, navigate::today())
            }),
            HomeTarget::DescribeTransaction => Err(not_yet("describing a transaction")),
            HomeTarget::PendingIms => Err(not_yet("the invoice management system list")),
            HomeTarget::ReviewQueue => Ok(Box::new(ReviewScreen::default()) as Box<dyn Screen>),
            HomeTarget::AiSettings => navigate::screen_for_key(navigate::AI_SETTINGS_KEY, ctx)
                .unwrap_or_else(|| Err(not_yet("AI and privacy"))),
            HomeTarget::ImportFromAnotherApp => navigate::screen_for_key(navigate::IMPORT_KEY, ctx)
                .unwrap_or_else(|| Err(not_yet("import from another app"))),
            HomeTarget::OutstandingReceivables => {
                navigate::open_report(ReportKind::Outstanding { receivable: true }, ctx)
            }
        };
        match opened {
            Ok(screen) => {
                self.notice = None;
                Some(screen)
            }
            Err(e) => {
                self.notice = Some(e.to_string());
                None
            }
        }
    }
}

/// The loaded books, or why there are none.
fn books(ctx: &AppCtx) -> Result<&Books, NavError> {
    ctx.books.as_ref().ok_or(NavError::NoCompany)
}

/// A target with no screen yet.
fn not_yet(what: &str) -> NavError {
    NavError::Unsupported {
        what: what.to_owned(),
        reason: "there is no screen for it yet".to_owned(),
    }
}

/// The company's financial year.
fn year_of(books: &Books) -> FinancialYear {
    FinancialYear {
        start: books.company.fy_start,
    }
}

/// The first active voucher type of `kind`, by id.
fn type_of(books: &Books, kind: BaseVoucherKind) -> Result<ledgerly_core::VoucherTypeId, NavError> {
    books
        .masters
        .voucher_types
        .values()
        .find(|t| t.base == kind && t.active)
        .map(|t| t.id)
        .ok_or_else(|| NavError::Unsupported {
            what: format!("a new {kind:?} voucher"),
            reason: "the company has no active voucher type of this kind".to_owned(),
        })
}

/// Voucher entry for a new voucher of `kind`, dated `today`.
fn entry_screen(
    books: &Books,
    kind: BaseVoucherKind,
    today: Date,
) -> Result<Box<dyn Screen>, NavError> {
    let id = type_of(books, kind)?;
    match VoucherEntryScreen::open(books, id, year_of(books), today) {
        Ok(Outcome::Value(screen)) => Ok(Box::new(screen)),
        Ok(Outcome::Unsupported(u)) => Err(NavError::Unsupported {
            what: u.what,
            reason: u.reason,
        }),
        Err(e) => Err(NavError::Unsupported {
            what: format!("a new {kind:?} voucher"),
            reason: e.to_string(),
        }),
    }
}

/// The GST screen a Home link to GST returns opens, on the tab the link names.
///
/// Tab names, as [`HomeTarget::GstReturns`] carries them: `"gstr1"` (GSTR-1), `"gstr3b"`
/// (GSTR-3B) and `"gstr2b-ims"` (2B and IMS reconciliation). The Home tile names `"gstr3b"`; the
/// G key and the palette send the empty name, which keeps the screen on its default tab. Any
/// other name is refused rather than opened on a guessed tab.
///
/// # Errors
/// [`NavError::Unsupported`] for an unknown tab name or for a target that is not GST returns;
/// otherwise the errors of [`navigate::gst_screen`].
pub fn gst_screen_for_target(books: &Books, target: &HomeTarget) -> Result<GstScreen, NavError> {
    let HomeTarget::GstReturns { tab } = target else {
        return Err(NavError::Unsupported {
            what: "GST returns".to_owned(),
            reason: "the link does not name GST returns".to_owned(),
        });
    };
    let wanted = match tab.as_str() {
        "" => None,
        "gstr1" => Some(GstTab::Gstr1),
        "gstr3b" => Some(GstTab::Gstr3b),
        "gstr2b-ims" => Some(GstTab::Reconciliation),
        other => {
            return Err(NavError::Unsupported {
                what: format!("the GST return tab '{other}'"),
                reason: "the GST screen has no tab with this name".to_owned(),
            });
        }
    };
    let mut screen = navigate::gst_screen(books)?;
    if let Some(tab) = wanted {
        screen.switch_tab(tab);
    }
    Ok(screen)
}

impl Screen for GatewayScreen {
    fn title(&self, tr: &Tr) -> String {
        self.home.title(tr)
    }

    fn page_title(&self, tr: &Tr) -> String {
        self.home.page_title(tr)
    }

    fn key_bar_name(&self, tr: &Tr) -> String {
        self.home.key_bar_name(tr)
    }

    fn subtitle(&self, tr: &Tr) -> Option<String> {
        self.home.subtitle(tr)
    }

    fn keys(&self, tr: &Tr) -> Vec<KeyHint> {
        self.home.keys(tr)
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut AppCtx) -> ScreenAction {
        // What opening the company did (an upgrade, or a backup that failed) is shown first.
        if let Some(note) = crate::open_company::take_pending() {
            return ScreenAction::Push(Box::new(crate::screens::backup::BackupScreen::after_open(
                note,
            )));
        }
        if let Some(books) = ctx.books.as_ref() {
            self.follow(books);
        } else {
            ui.label("Loading the company's books...");
            ui.ctx().request_repaint();
        }
        if let Some(notice) = &self.notice {
            ui.colored_label(ui.visuals().error_fg_color, notice.as_str());
        }
        let _stay = self.home.ui(ui, ctx);
        match self.home.take_request() {
            Some(target) => self
                .open(&target, ctx)
                .map_or(ScreenAction::Stay, ScreenAction::Push),
            None => ScreenAction::Stay,
        }
    }
}
