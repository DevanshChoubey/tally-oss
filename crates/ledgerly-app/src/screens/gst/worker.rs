//! Worker mode of the GST screen: GSTR-1 and GSTR-3B come from `Api::gstr1` and `Api::gstr3b`,
//! run on the API worker, and are shown as the engine returned them (rule C4).
//!
//! * An `Unsupported` answer is shown with [`unsupported_text`](crate::screens::wire::unsupported_text)
//!   and no figure.
//! * A failed call is shown as unsupported with the error text, and kept in
//!   [`GstScreen::load_error`]; no table is built from a failed call.
//! * The screen sums nothing. The figures are rows the return already holds: the B2B invoice
//!   values and the HSN summary (Table 12) for GSTR-1, and the 3.1(a) and 4(A)(5) amounts for
//!   GSTR-3B. A GSTR-1 table this screen does not draw is named in [`Figures::notes`], never
//!   dropped silently (rule A9 in spirit: nothing disappears without a word).

use ledgerly_api::{Api, Bucket as ApiBucket, BucketedVoucher, Gstr1Return, Gstr3bWorking};
use ledgerly_core::{Gstin, Outcome, Paise, Period, Unsupported};

use super::{Bucket, ExceptionItem, Feed, GstScreen, GstTab, date_cell};
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::worker::ApiWorker;

/// One figure of a return, with the words that name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigureRow {
    /// What the figure is, for example `Table 3.1(a) taxable value`.
    pub label: String,
    /// The amount, exactly as the return holds it.
    pub amount: Paise,
}

/// The figures of one return, and the parts of it this screen does not draw.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Figures {
    /// The figures, in display order.
    pub rows: Vec<FigureRow>,
    /// Lines about what is not shown here, or about the return as a whole.
    pub notes: Vec<String>,
}

/// What the worker brings back for one period.
#[derive(Debug)]
struct GstAnswer {
    gstr1: Result<Outcome<Gstr1Return>, String>,
    gstr3b: Result<Outcome<Gstr3bWorking>, String>,
}

/// The load a worker-mode screen has asked for and the call that is running.
#[derive(Debug, Default)]
pub(super) struct GstRun {
    want: Option<(Gstin, Period)>,
    latest: Latest<GstAnswer>,
    error: Option<String>,
    /// The return last asked for, which the JSON export builds from.
    scope: Option<(Gstin, Period)>,
    /// The user asked for the GSTR-1 JSON; the next tick starts it.
    export_want: bool,
    /// The GSTR-1 JSON call (`Api::gstr1_json`), which the engine refuses while a voucher is held.
    export: Latest<Result<Outcome<String>, String>>,
    /// The file name the JSON is saved under, fixed when the call starts.
    export_name: String,
}

fn fetch(api: &Api, gstin: &Gstin, period: Period) -> GstAnswer {
    GstAnswer {
        gstr1: api.gstr1(gstin, period).map_err(|e| e.to_string()),
        gstr3b: api.gstr3b(gstin, period).map_err(|e| e.to_string()),
    }
}

impl GstScreen {
    /// Asks for GSTR-1 and GSTR-3B of `gstin` for `period`. The next [`GstScreen::tick`] sends
    /// the request to the API worker.
    pub fn request_load(&mut self, gstin: Gstin, period: Period) {
        self.run.scope = Some((gstin.clone(), period));
        self.run.want = Some((gstin, period));
    }

    /// Asks for the GSTR-1 JSON of the loaded return. The next [`GstScreen::tick`] builds it, and
    /// the screen then offers to save it. Nothing is checked here; see
    /// [`GstScreen::export_blocked_reason`] for the checks the screen makes first.
    pub(super) fn request_json(&mut self) {
        self.run.export_want = true;
    }

    /// Why the GSTR-1 JSON cannot be exported now, or `None` when it can be asked for: the tab is
    /// not GSTR-1, no return is loaded, GSTR-1 is still loading, or the engine could not build it.
    /// The engine's own refusal (a voucher that must be fixed first) is not repeated here: the
    /// JSON call returns it, and it is shown as a note.
    #[must_use]
    pub fn export_blocked_reason(&self) -> Option<String> {
        if self.tab != GstTab::Gstr1 {
            return Some(
                "GSTR-1 JSON is exported from the GSTR-1 tab. Press 1 to open it.".to_owned(),
            );
        }
        if self.run.scope.is_none() {
            return Some("No return is loaded, so there is no JSON to export.".to_owned());
        }
        match &self.gstr1 {
            Feed::NotLoaded => Some("GSTR-1 is still loading, so there is no JSON yet.".to_owned()),
            Feed::Unsupported(u) => Some(unsupported_text(u)),
            Feed::Loaded(_) => None,
        }
    }

    /// The JSON the user asked for, with its file name, once it is built.
    pub fn take_json_export(&mut self) -> Option<(String, String)> {
        self.export_json.take()
    }

    /// `true` while a load waits for the worker or runs on it.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        self.run.want.is_some() || self.run.latest.is_pending()
    }

    /// Why the last load failed (an error, not an unsupported return), when it did.
    #[must_use]
    pub fn load_error(&self) -> Option<&str> {
        self.run.error.as_deref()
    }

    /// The figures of the GSTR-1 or GSTR-3B tab; `None` for the reconciliation tab.
    #[must_use]
    pub fn figures(&self, tab: GstTab) -> Option<&Figures> {
        match tab {
            GstTab::Gstr1 => Some(&self.figures1),
            GstTab::Gstr3b => Some(&self.figures3b),
            GstTab::Reconciliation => None,
        }
    }

    /// The text shown in place of the `tab` table when the engine could not build it, written
    /// by [`unsupported_text`]. `None` when the tab has data, has not been loaded, or is the
    /// reconciliation tab.
    #[must_use]
    pub fn unsupported_line(&self, tab: GstTab) -> Option<String> {
        let feed = match tab {
            GstTab::Gstr1 => &self.gstr1,
            GstTab::Gstr3b => &self.gstr3b,
            GstTab::Reconciliation => return None,
        };
        match feed {
            Feed::Unsupported(u) => Some(unsupported_text(u)),
            Feed::NotLoaded | Feed::Loaded(_) => None,
        }
    }

    /// Advances the load by one step: shows a finished answer, then sends a waiting request to
    /// `worker`. With no worker the request is dropped and the screen says no company is open.
    pub fn tick(&mut self, worker: Option<&ApiWorker>) {
        match self.run.latest.poll() {
            Some(Done::Ready(answer)) => self.apply_answer(answer),
            Some(Done::Lost) => {
                self.run.error =
                    Some("the background worker stopped before the return was built".to_owned());
            }
            None => {}
        }
        self.tick_export(worker);
        let Some((gstin, period)) = self.run.want.take() else {
            return;
        };
        let Some(handle) = worker else {
            self.run.error = Some("no company is open, so no return can be built".to_owned());
            return;
        };
        self.run
            .latest
            .start(handle, move |api| fetch(api, &gstin, period));
    }

    /// Shows a finished JSON answer, then starts a requested JSON call on `worker`.
    fn tick_export(&mut self, worker: Option<&ApiWorker>) {
        match self.run.export.poll() {
            Some(Done::Ready(Ok(Outcome::Value(json)))) => {
                self.export_json = Some((self.run.export_name.clone(), json));
            }
            Some(Done::Ready(Ok(Outcome::Unsupported(u)))) => {
                self.notice = Some(unsupported_text(&u));
            }
            Some(Done::Ready(Err(message))) => {
                self.notice = Some(format!("The JSON could not be built: {message}"));
            }
            Some(Done::Lost) => {
                self.notice =
                    Some("The background worker stopped before the JSON was built.".to_owned());
            }
            None => {}
        }
        if !self.run.export_want {
            return;
        }
        self.run.export_want = false;
        let Some((gstin, period)) = self.run.scope.clone() else {
            self.notice = Some("No return is loaded, so there is no JSON to export.".to_owned());
            return;
        };
        let Some(handle) = worker else {
            self.notice = Some("No company is open, so no JSON can be built.".to_owned());
            return;
        };
        let from = period.from();
        self.run.export_name = format!(
            "gstr1-{:04}-{:02}.json",
            from.year(),
            u8::from(from.month())
        );
        self.run.export.start(handle, move |api| {
            api.gstr1_json(&gstin, period).map_err(|e| e.to_string())
        });
    }

    fn apply_answer(&mut self, answer: GstAnswer) {
        self.run.error = None;
        match answer.gstr1 {
            Ok(Outcome::Value(ret)) => {
                let items = ret.buckets.iter().map(exception_item).collect();
                self.set_gstr1(Feed::Loaded(items));
                self.figures1 = gstr1_figures(&ret);
            }
            Ok(Outcome::Unsupported(u)) => {
                self.set_gstr1(Feed::Unsupported(u));
                self.figures1 = Figures::default();
            }
            Err(message) => {
                self.set_gstr1(failed("GSTR-1", &message));
                self.figures1 = Figures::default();
                self.run.error = Some(format!("GSTR-1: {message}"));
            }
        }
        match answer.gstr3b {
            Ok(Outcome::Value(working)) => {
                self.set_gstr3b(Feed::Loaded(Vec::new()));
                self.figures3b = gstr3b_figures(&working);
            }
            Ok(Outcome::Unsupported(u)) => {
                self.set_gstr3b(Feed::Unsupported(u));
                self.figures3b = Figures::default();
            }
            Err(message) => {
                self.set_gstr3b(failed("GSTR-3B", &message));
                self.figures3b = Figures::default();
                self.run.error = Some(format!("GSTR-3B: {message}"));
            }
        }
    }
}

/// A failed call, shown the way an unsupported one is, with the error as the reason.
fn failed<T>(what: &str, message: &str) -> Feed<T> {
    Feed::Unsupported(Unsupported {
        what: format!("{what} could not be built"),
        reason: message.to_owned(),
    })
}

/// The screen's item for a voucher bucket the engine decided.
fn exception_item(b: &BucketedVoucher) -> ExceptionItem {
    let (bucket, default_detail) = match b.bucket {
        ApiBucket::Ready => (Bucket::Ready, "Its figures are in the return."),
        ApiBucket::NotRelevant => (Bucket::NotRelevant, "Not a GSTR-1 document."),
        ApiBucket::Mismatch => (Bucket::Mismatch, "No reason was given."),
        ApiBucket::NeedsCorrection => (Bucket::NeedsCorrection, "No reason was given."),
    };
    ExceptionItem {
        bucket,
        title: format!("Voucher {}", b.voucher.0),
        detail: b
            .reason
            .clone()
            .unwrap_or_else(|| default_detail.to_owned()),
        voucher: Some(b.voucher),
    }
}

fn row(rows: &mut Vec<FigureRow>, label: String, amount: Paise) {
    rows.push(FigureRow { label, amount });
}

fn note_if_present(notes: &mut Vec<String>, table: &str, count: usize) {
    if count > 0 {
        notes.push(format!(
            "{table}: {count} entr{} in the return, not drawn on this screen.",
            if count == 1 { "y" } else { "ies" }
        ));
    }
}

/// Table 4 invoice values and the Table 12 HSN summary, read from the return.
fn gstr1_figures(ret: &Gstr1Return) -> Figures {
    let mut rows = Vec::new();
    for party in &ret.gstr1.b2b {
        for inv in &party.invoices {
            row(
                &mut rows,
                format!(
                    "Table 4 B2B, {}, invoice {} of {}: invoice value",
                    party.ctin,
                    inv.num,
                    date_cell(Some(inv.date))
                ),
                inv.value,
            );
        }
    }
    for hsn in &ret.gstr1.hsn {
        let head = format!("Table 12 HSN {} at {}%", hsn.hsn.as_str(), hsn.rate.0);
        row(&mut rows, format!("{head}: taxable value"), hsn.taxable);
        row(&mut rows, format!("{head}: integrated tax"), hsn.igst);
        row(&mut rows, format!("{head}: central tax"), hsn.cgst);
        row(&mut rows, format!("{head}: state/UT tax"), hsn.sgst);
        row(&mut rows, format!("{head}: cess"), hsn.cess);
    }
    let mut notes = Vec::new();
    note_if_present(&mut notes, "Table 5 B2CL", ret.gstr1.b2cl.len());
    note_if_present(&mut notes, "Table 7 B2CS", ret.gstr1.b2cs.len());
    note_if_present(&mut notes, "Table 9B CDNR", ret.gstr1.cdnr.len());
    note_if_present(&mut notes, "Table 9B CDNUR", ret.gstr1.cdnur.len());
    note_if_present(&mut notes, "Table 6A exports", ret.gstr1.exp.len());
    note_if_present(
        &mut notes,
        "Table 8 nil, exempt and non-GST",
        ret.gstr1.nil.len(),
    );
    note_if_present(
        &mut notes,
        "Table 13 documents issued",
        ret.gstr1.doc_issue.len(),
    );
    Figures { rows, notes }
}

/// The four tax heads of one table, as figure rows.
fn tax_heads(rows: &mut Vec<FigureRow>, table: &str, heads: [Paise; 4]) {
    let [integrated, central, state_ut, cess] = heads;
    row(rows, format!("{table}: integrated tax"), integrated);
    row(rows, format!("{table}: central tax"), central);
    row(rows, format!("{table}: state/UT tax"), state_ut);
    row(rows, format!("{table}: cess"), cess);
}

/// Tables 3.1(a), 4(A)(5) and the credit that is not claimable, read from the working.
fn gstr3b_figures(w: &Gstr3bWorking) -> Figures {
    let mut rows = Vec::new();
    row(
        &mut rows,
        "Table 3.1(a): taxable value".to_owned(),
        w.outward_taxable_value,
    );
    let o = w.outward_tax;
    tax_heads(
        &mut rows,
        "Table 3.1(a)",
        [o.integrated, o.central, o.state_ut, o.cess],
    );
    let i = w.itc_all_other;
    tax_heads(
        &mut rows,
        "Table 4(A)(5) all other ITC",
        [i.integrated, i.central, i.state_ut, i.cess],
    );
    let n = w.itc_not_claimable;
    tax_heads(
        &mut rows,
        "Credit not claimable (not in 4(A)(5))",
        [n.integrated, n.central, n.state_ut, n.cess],
    );
    let mut notes = vec![format!("Rule pack version {}.", w.pack_version)];
    if !w.reverse_charge_purchases.is_empty() {
        notes.push(format!(
            "{} purchase(s) under reverse charge are in neither 3.1(a) nor 4(A)(5).",
            w.reverse_charge_purchases.len()
        ));
    }
    Figures { rows, notes }
}
