//! The rules of the company setup wizard (mock `SCREENS.setup`, `design/mock-v2/index.html`
//! 1855-1895), with no drawing in them, so the tests can check them.
//!
//! What the wizard may save is decided here: a feature switch is offered only where the model has
//! one ([`feature_rows`]); a GSTIN must agree with the state ([`check_gstin_in`]); books may begin
//! on any day of the financial year ([`parse_books_begin_in`]).

use std::path::Path;

use ledgerly_api::{AiTier, Api};
use ledgerly_core::{Clock, CompanyId, Date, Gstin, GstinError, Outcome, Pan, StateCode};
use ledgerly_model::{
    Address, Company, EntityKind, Features, GstRegistration, NegativeStockPolicy,
};
use time::Month;

use crate::i18n::Tr;

/// One step of the wizard, in the order the mock's step rail draws them (index.html:1855).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Welcome: create, import, open or try the sample.
    Welcome,
    /// Company details: name, type, GSTIN, state, financial year and books date.
    Company,
    /// The features the company uses.
    Features,
    /// The keyboard preset.
    Keyboard,
    /// AI and privacy.
    Ai,
    /// Backups.
    Backup,
    /// The company is ready.
    Done,
}

impl Step {
    /// Every step, in rail order.
    pub const ALL: [Step; 7] = [
        Step::Welcome,
        Step::Company,
        Step::Features,
        Step::Keyboard,
        Step::Ai,
        Step::Backup,
        Step::Done,
    ];

    /// The step's place on the rail, from 0.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Step::Welcome => 0,
            Step::Company => 1,
            Step::Features => 2,
            Step::Keyboard => 3,
            Step::Ai => 4,
            Step::Backup => 5,
            Step::Done => 6,
        }
    }

    /// The mock's id for the step (`setup.<key>`).
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Step::Welcome => "welcome",
            Step::Company => "company",
            Step::Features => "features",
            Step::Keyboard => "keyboard",
            Step::Ai => "ai",
            Step::Backup => "backup",
            Step::Done => "done",
        }
    }

    /// The step's name in English, as the rail shows it.
    #[must_use]
    pub fn label(self) -> String {
        step_label_in(self, &Tr::default())
    }

    /// The step after this one, or `None` on the last.
    #[must_use]
    pub fn next(self) -> Option<Step> {
        Self::ALL.get(self.index() + 1).copied()
    }

    /// The step before this one, or `None` on the first.
    #[must_use]
    pub fn back(self) -> Option<Step> {
        self.index()
            .checked_sub(1)
            .and_then(|i| Self::ALL.get(i).copied())
    }
}

/// The step's name in the words of `tr`, as the step rail shows it (index.html:1855).
#[must_use]
pub fn step_label_in(step: Step, tr: &Tr) -> String {
    tr.t(&format!("setup.step.{}", step.key()))
}

/// A key the wizard looks at (index.html:1887-1888).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyInput {
    /// Enter: continue.
    Enter,
    /// Escape: back one step.
    Escape,
}

/// What the wizard does with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Go to the next step (on Done, that is Home).
    Continue,
    /// Go to the previous step.
    Back,
    /// Not the wizard's key here; the shell or the focused field keeps it.
    Pass,
}

/// What a step says when its choice cannot be stored yet, or `None` when the step saves what it
/// asks. The keyboard preset and backup folder have no store (design/PARITY-GAPS.md); the AI tier
/// is stored, and [`ai_notice`] covers the one AI choice the engine refuses.
#[must_use]
pub fn unsaved_notice(step: Step) -> Option<&'static str> {
    match step {
        Step::Keyboard => Some(
            "This choice is not saved yet: every company uses the classic keys until keyboard presets are stored (PARITY-GAPS).",
        ),
        Step::Backup => {
            Some("This folder is not saved and no backup runs on a schedule yet (PARITY-GAPS).")
        }
        Step::Welcome | Step::Company | Step::Ai | Step::Features | Step::Done => None,
    }
}

/// What a step says when its choice cannot be stored yet, or `None` when the step saves what it
/// asks, in the words of `tr`. The keyboard preset and backup folder have no store
/// (design/PARITY-GAPS.md); the AI tier is stored, and [`ai_notice_in`] covers the one AI choice
/// the engine refuses.
#[must_use]
pub fn unsaved_notice_in(step: Step, tr: &Tr) -> Option<String> {
    match step {
        Step::Keyboard => Some(tr.t("setup.notice.keyboard_unsaved")),
        Step::Backup => Some(tr.t("setup.notice.backup_unsaved")),
        Step::Welcome | Step::Company | Step::Ai | Step::Features | Step::Done => None,
    }
}

/// What the AI step says under the cards for `choice`, or `None` when the choice is stored with
/// the company. Off and On this computer are stored ([`create_company`]); the cloud tier is
/// refused by the engine until a keychain store exists (design/PARITY-GAPS.md).
#[must_use]
pub fn ai_notice(choice: AiTier) -> Option<&'static str> {
    match choice {
        AiTier::Off | AiTier::Local => None,
        AiTier::Cloud => Some(
            "Cloud is not saved yet: this build has no keychain store for the provider key. The company starts with AI not set up (PARITY-GAPS).",
        ),
    }
}

/// What the AI step says under the cards for `choice`, in the words of `tr`, or `None` when the
/// choice is stored with the company. Off and On this computer are stored ([`create_company_in`]);
/// the cloud tier is refused by the engine until a keychain store exists (design/PARITY-GAPS.md).
#[must_use]
pub fn ai_notice_in(choice: AiTier, tr: &Tr) -> Option<String> {
    match choice {
        AiTier::Off | AiTier::Local => None,
        AiTier::Cloud => Some(tr.t("setup.notice.cloud_unsaved")),
    }
}

/// Creates the company file, with English notes.
///
/// # Errors
/// The reason, in the words of the translation file, the value cannot be made.
pub fn create_company(
    path: &Path,
    company: &Company,
    clock: Clock,
    ai: AiTier,
) -> Result<(Api, Option<String>), String> {
    create_company_in(path, company, clock, ai, &Tr::default())
}

/// Creates the company file at `path` and stores the AI tier chosen in the wizard. Returns the
/// open company and, when the tier could not be stored, what the engine said, for the Done step.
/// The company is kept either way: a refused tier leaves AI not set up.
///
/// # Errors
/// The reason the company file could not be created.
pub fn create_company_in(
    path: &Path,
    company: &Company,
    clock: Clock,
    ai: AiTier,
    tr: &Tr,
) -> Result<(Api, Option<String>), String> {
    let mut api = Api::create(path, company, clock).map_err(|e| e.to_string())?;
    let note = match api.set_ai_tier(ai) {
        Ok(Outcome::Value(())) => None,
        Ok(Outcome::Unsupported(refused)) => Some(tr.tf(
            "setup.notice.ai_not_saved",
            &[("reason", refused.reason.as_str())],
        )),
        Err(e) => Some(tr.tf(
            "setup.notice.ai_not_saved",
            &[("reason", e.to_string().as_str())],
        )),
    };
    Ok((api, note))
}

/// What a key does on `step`. Enter and Escape do nothing while a field has focus (index.html:1887
/// and 1888 check `isTyping()`), and Escape does nothing on the first step or on Done.
#[must_use]
pub fn step_outcome(step: Step, typing: bool, key: KeyInput) -> KeyOutcome {
    match key {
        KeyInput::Enter => {
            if typing {
                KeyOutcome::Pass
            } else {
                KeyOutcome::Continue
            }
        }
        // Done cannot go back: the company exists, and going back would create a second one.
        KeyInput::Escape => match (typing, step, step.back()) {
            (false, Step::Done, _) | (false, _, None) | (true, _, _) => KeyOutcome::Pass,
            (false, _, Some(_)) => KeyOutcome::Back,
        },
    }
}

/// What the GSTIN field says about the state chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GstinCheck {
    /// The field is empty: a business may not be registered yet, so it is allowed.
    Blank,
    /// Not a GSTIN (length, characters, PAN shape or check character). The reason is shown.
    Invalid(String),
    /// A GSTIN whose first two digits name another state than the one chosen. The message names
    /// the fix (index.html:1868).
    Mismatch {
        /// The state the GSTIN names.
        found: StateCode,
        /// The state the form has.
        chosen: StateCode,
        /// The sentence the field shows.
        message: String,
    },
    /// A GSTIN that agrees with the state.
    Valid(Gstin),
}

/// Checks the GSTIN text against the state, with English reasons.
#[must_use]
pub fn check_gstin(text: &str, chosen: StateCode) -> GstinCheck {
    check_gstin_in(text, chosen, &Tr::default())
}

/// Checks the GSTIN text against the state the form has. A valid GSTIN names its state through
/// its first two digits ([`Gstin::state`]); the caller fills the state from it.
#[must_use]
pub fn check_gstin_in(text: &str, chosen: StateCode, tr: &Tr) -> GstinCheck {
    if text.trim().is_empty() {
        return GstinCheck::Blank;
    }
    let gstin = match Gstin::parse(text) {
        Ok(gstin) => gstin,
        Err(e) => return GstinCheck::Invalid(gstin_reason(&e)),
    };
    let Some(found) = gstin.state() else {
        return GstinCheck::Invalid(tr.t("setup.notice.gstin_state_code"));
    };
    if found == chosen {
        return GstinCheck::Valid(gstin);
    }
    let found_code = format!("{:02}", found.get());
    let chosen_code = format!("{:02}", chosen.get());
    let message = tr.tf(
        "setup.notice.gstin_mismatch",
        &[
            ("prefix", found_code.as_str()),
            ("prefix_name", found.name()),
            ("state", chosen_code.as_str()),
            ("state_name", chosen.name()),
        ],
    );
    GstinCheck::Mismatch {
        found,
        chosen,
        message,
    }
}

/// The reason a GSTIN is not accepted, as the field shows it.
fn gstin_reason(error: &GstinError) -> String {
    error.to_string()
}

/// The state a GSTIN fills in, when the text is a GSTIN (index.html:1868 "fills the state").
#[must_use]
pub fn state_from_gstin(text: &str) -> Option<StateCode> {
    Gstin::parse(text).ok().and_then(|g| g.state())
}

/// `true` when the GSTIN check lets Continue go on (index.html:1861 disables it otherwise).
#[must_use]
pub fn gstin_allows_continue(check: &GstinCheck) -> bool {
    match check {
        GstinCheck::Blank | GstinCheck::Valid(_) => true,
        GstinCheck::Invalid(_) | GstinCheck::Mismatch { .. } => false,
    }
}

/// The financial year start, with English reasons.
///
/// # Errors
/// The reason, in the words of the translation file, the value cannot be made.
pub fn financial_year_start(today: Date) -> Result<Date, String> {
    financial_year_start_in(today, &Tr::default())
}

/// The financial year's first day for `today`: 1 April of the year that contains `today`
/// (Indian financial year, index.html:1870 "1 Apr 2026 to 31 Mar 2027").
///
/// # Errors
/// A message when the date is outside the calendar range.
pub fn financial_year_start_in(today: Date, tr: &Tr) -> Result<Date, String> {
    let year = if u8::from(today.month()) >= 4 {
        today.year()
    } else {
        today.year() - 1
    };
    Date::from_calendar_date(year, Month::April, 1).map_err(|_| tr.t("setup.notice.date_range"))
}

/// The financial year end, with English reasons.
///
/// # Errors
/// The reason, in the words of the translation file, the value cannot be made.
pub fn financial_year_end(fy_start: Date) -> Result<Date, String> {
    financial_year_end_in(fy_start, &Tr::default())
}

/// The last day of the financial year that starts on `fy_start` (31 March of the next year).
///
/// # Errors
/// A message when the date is outside the calendar range.
pub fn financial_year_end_in(fy_start: Date, tr: &Tr) -> Result<Date, String> {
    Date::from_calendar_date(fy_start.year() + 1, Month::March, 31)
        .map_err(|_| tr.t("setup.notice.date_range"))
}

/// Reads the books begin date, with English reasons.
///
/// # Errors
/// The reason, in the words of the translation file, the value cannot be made.
pub fn parse_books_begin(text: &str, fy_start: Date, fy_end: Date) -> Result<Date, String> {
    parse_books_begin_in(text, fy_start, fy_end, &Tr::default())
}

/// Reads the "Books begin from" text, `YYYY-MM-DD`. Books may begin on any day of the financial
/// year (Tally help, read 2026-11-23: change the date to the actual start of transactions).
///
/// # Errors
/// A message when the text is not a date, or the date is outside `fy_start` to `fy_end`.
pub fn parse_books_begin_in(
    text: &str,
    fy_start: Date,
    fy_end: Date,
    tr: &Tr,
) -> Result<Date, String> {
    let parts: Vec<&str> = text.trim().split('-').collect();
    let [year, month, day] = parts.as_slice() else {
        return Err(tr.t("setup.notice.date_format"));
    };
    let not_a_date = || tr.t("setup.notice.date_format");
    let year: i32 = year.parse().map_err(|_| not_a_date())?;
    let month: u8 = month.parse().map_err(|_| not_a_date())?;
    let day: u8 = day.parse().map_err(|_| not_a_date())?;
    let month = Month::try_from(month).map_err(|_| not_a_date())?;
    let date = Date::from_calendar_date(year, month, day).map_err(|_| not_a_date())?;
    if date < fy_start || date > fy_end {
        return Err(tr.tf(
            "setup.notice.books_between",
            &[
                ("from", fy_start.to_string().as_str()),
                ("to", fy_end.to_string().as_str()),
            ],
        ));
    }
    Ok(date)
}

/// One feature row of the wizard (index.html:1872-1880).
#[derive(Clone, Copy)]
pub struct FeatureRow {
    /// The key the translated words are looked up by: `setup.feature.<key>.label`, `.hint` and
    /// `.reason`.
    pub key: &'static str,
    /// The row's title in English (the screen draws the words of the files).
    pub label: &'static str,
    /// The one-line explanation the mock gives.
    pub hint: &'static str,
    /// On by default in the mock (index.html:1872).
    pub default_on: bool,
    /// The model switch this row turns on, when the model has one.
    pub switch: Option<fn(&mut Features) -> &mut bool>,
    /// Why the row cannot be turned on, when the model has no switch for it.
    pub unavailable: Option<&'static str>,
}

/// The feature rows of the wizard. A row is a working switch only when the model has the switch
/// (`ledgerly_model::Features`); the rest say why they are not available.
#[must_use]
pub fn feature_rows() -> Vec<FeatureRow> {
    vec![
        FeatureRow {
            key: "inventory",
            label: "Inventory",
            hint: "Stock items, quantities and stock value in the balance sheet.",
            default_on: true,
            switch: Some(|f| &mut f.inventory),
            unavailable: None,
        },
        FeatureRow {
            key: "gst",
            label: "GST",
            hint: "Tax on invoices, GSTR-1 and GSTR-3B data, 2B matching.",
            default_on: true,
            switch: Some(|f| &mut f.gst),
            unavailable: None,
        },
        FeatureRow {
            key: "bank",
            label: "Bank reconciliation",
            hint: "Import statements and match lines to vouchers.",
            default_on: true,
            switch: Some(|f| &mut f.bank_reconciliation),
            unavailable: None,
        },
        FeatureRow {
            key: "bill_wise",
            label: "Bill-wise tracking",
            hint: "Track each invoice until it is paid; ageing and reminders.",
            default_on: true,
            switch: Some(|f| &mut f.bill_wise),
            unavailable: None,
        },
        FeatureRow {
            key: "e_invoice",
            label: "e-Invoice JSON",
            hint: "Export invoices in the government schema. You upload them yourself.",
            default_on: true,
            switch: None,
            unavailable: Some(
                "The model has no e-invoice switch yet, and e-invoice export is not built (PARITY-GAPS).",
            ),
        },
        FeatureRow {
            key: "currencies",
            label: "Multiple currencies",
            hint: "For export and import. Off for most businesses.",
            default_on: false,
            switch: None,
            unavailable: Some(
                "The model has no currency switch yet, and foreign currency is not built (PARITY-GAPS).",
            ),
        },
        FeatureRow {
            key: "cost_centres",
            label: "Cost centres",
            hint: "Split expenses by branch or project.",
            default_on: false,
            switch: Some(|f| &mut f.cost_centres),
            unavailable: None,
        },
    ]
}

/// The features the wizard starts with: the default-on rows that have a model switch.
#[must_use]
pub fn default_features() -> Features {
    let mut features = Features::default();
    for row in feature_rows() {
        if let (true, Some(switch)) = (row.default_on, row.switch) {
            *switch(&mut features) = true;
        }
    }
    features
}

/// What the company step holds, once the user has typed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanyForm {
    /// The company's name (mock "Company name").
    pub name: String,
    /// The company's type (mock "Type").
    pub kind: EntityKind,
    /// The city, kept as the first address line (mock "City").
    pub city: String,
    /// The GSTIN text; may be blank.
    pub gstin: String,
    /// The state the form has.
    pub state: StateCode,
    /// The financial year's first day.
    pub fy_start: Date,
    /// The day the books begin, inside the financial year.
    pub books_begin: Date,
}

/// Builds the company the form describes, with English reasons.
///
/// # Errors
/// The reason, in the words of the translation file, the value cannot be made.
pub fn build_company(form: &CompanyForm, features: Features) -> Result<Company, String> {
    build_company_in(form, features, &Tr::default())
}

/// Builds the company the form describes, with the features chosen. A GSTIN that does not agree
/// with the state is refused, as the form refuses it.
///
/// # Errors
/// A message naming the field that is not acceptable.
pub fn build_company_in(
    form: &CompanyForm,
    features: Features,
    tr: &Tr,
) -> Result<Company, String> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(tr.t("setup.notice.type_name"));
    }
    let (registrations, pan) = match check_gstin_in(&form.gstin, form.state, tr) {
        GstinCheck::Blank => (Vec::new(), None),
        GstinCheck::Valid(gstin) => {
            let pan = Pan::parse(gstin.pan_str()).map_err(|e| e.to_string())?;
            let registration = GstRegistration {
                gstin,
                state: form.state,
                // derived: the registration is taken to apply from the first day of the year.
                effective_from: form.fy_start,
                composition: false,
            };
            (vec![registration], Some(pan))
        }
        GstinCheck::Invalid(reason) => return Err(reason),
        GstinCheck::Mismatch { message, .. } => return Err(message),
    };
    let lines = if form.city.trim().is_empty() {
        Vec::new()
    } else {
        vec![form.city.trim().to_owned()]
    };
    Ok(Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: name.to_owned(),
        mailing_name: None,
        address: Address {
            lines,
            pincode: None,
            state: Some(form.state),
            country: String::new(),
        },
        state: form.state,
        pan,
        registrations,
        fy_start: form.fy_start,
        books_begin: form.books_begin,
        entity_kind: form.kind,
        features,
        negative_stock: NegativeStockPolicy::Block,
    })
}
