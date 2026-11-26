//! The rules of the company setup wizard (mock `SCREENS.setup`, `design/mock-v2/index.html`
//! 1855-1895), with no drawing in them, so the tests can check them.
//!
//! What the wizard may save is decided here: a feature switch is offered only where the model has
//! one ([`feature_rows`]); a GSTIN must agree with the state ([`check_gstin`]); books may begin
//! on any day of the financial year ([`parse_books_begin`]).

use std::path::Path;

use crate::i18n::Tr;

use ledgerly_api::{AiTier, Api, GstFiling};
use ledgerly_core::{Clock, CompanyId, Date, Gstin, GstinError, Outcome, Pan, StateCode};
use ledgerly_model::{
    Address, Company, EntityKind, Features, GstRegistration, NegativeStockPolicy,
};
use time::Month;

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

    /// The label the step rail shows (index.html:1855), in the current language.
    #[must_use]
    pub fn label(self) -> String {
        step_label_in(self, &crate::i18n::current())
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
/// asks, in the current language. The keyboard preset and backup folder have no store
/// (design/PARITY-GAPS.md); the AI tier is stored, and [`ai_notice`] covers the one AI choice the
/// engine refuses.
#[must_use]
pub fn unsaved_notice(step: Step) -> Option<String> {
    unsaved_notice_in(step, &crate::i18n::current())
}

/// [`unsaved_notice`] in the words of `tr`.
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
pub fn ai_notice(choice: AiTier) -> Option<String> {
    match choice {
        AiTier::Off | AiTier::Local => None,
        AiTier::Cloud => Some(crate::i18n::t("setup.notice.cloud_unsaved")),
    }
}

/// Creates the company file at `path` and stores the AI tier chosen in the wizard. Returns the
/// open company and, when the tier could not be stored, what the engine said, for the Done step.
/// The company is kept either way: a refused tier leaves AI not set up.
///
/// # Errors
/// The reason the company file could not be created.
pub fn create_company(
    path: &Path,
    company: &Company,
    clock: Clock,
    ai: AiTier,
) -> Result<(Api, Option<String>), String> {
    // The default file lives in `~/Documents/Ledgerly/`, which a new computer does not have yet.
    if let Some(folder) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(folder).map_err(|e| {
            crate::i18n::tf(
                "setup.notice.folder_failed",
                &[
                    ("folder", &folder.display().to_string()),
                    ("error", &e.to_string()),
                ],
            )
        })?;
    }
    let mut api = Api::create(path, company, clock).map_err(|e| e.to_string())?;
    let note = match api.set_ai_tier(ai) {
        Ok(Outcome::Value(())) => None,
        Ok(Outcome::Unsupported(refused)) => Some(crate::i18n::tf(
            "setup.notice.ai_not_saved",
            &[("reason", &refused.reason)],
        )),
        Err(e) => Some(crate::i18n::tf(
            "setup.notice.ai_not_saved",
            &[("reason", &e.to_string())],
        )),
    };
    Ok((api, note))
}

/// Like [`create_company`], and also stores the GST filing frequency chosen on the company step.
///
/// # Errors
/// The reason the company file could not be created.
pub fn create_company_with_filing(
    path: &Path,
    company: &Company,
    clock: Clock,
    ai: AiTier,
    filing: GstFiling,
) -> Result<(Api, Option<String>), String> {
    let (mut api, note) = create_company(path, company, clock, ai)?;
    let note = match api.set_gst_filing(filing) {
        Ok(Outcome::Value(())) => note,
        Ok(Outcome::Unsupported(refused)) => Some(crate::i18n::tf(
            "setup.notice.filing_not_saved",
            &[("reason", &refused.reason)],
        )),
        Err(e) => Some(crate::i18n::tf(
            "setup.notice.filing_not_saved",
            &[("reason", &e.to_string())],
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

/// Checks the GSTIN text against the state the form has. A valid GSTIN names its state through
/// its first two digits ([`Gstin::state`]); the caller fills the state from it.
#[must_use]
pub fn check_gstin(text: &str, chosen: StateCode) -> GstinCheck {
    if text.trim().is_empty() {
        return GstinCheck::Blank;
    }
    let gstin = match Gstin::parse(text) {
        Ok(gstin) => gstin,
        Err(e) => return GstinCheck::Invalid(gstin_reason(&e)),
    };
    let Some(found) = gstin.state() else {
        return GstinCheck::Invalid(crate::i18n::t("setup.notice.gstin_state_code"));
    };
    if found == chosen {
        return GstinCheck::Valid(gstin);
    }
    let message = crate::i18n::tf(
        "setup.notice.gstin_mismatch",
        &[
            ("prefix", &format!("{:02}", found.get())),
            ("prefix_name", found.name()),
            ("state", &format!("{:02}", chosen.get())),
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

/// The financial year's first day for `today`: 1 April of the year that contains `today`
/// (Indian financial year, index.html:1870 "1 Apr 2026 to 31 Mar 2027").
///
/// # Errors
/// A message when the date is outside the calendar range.
pub fn financial_year_start(today: Date) -> Result<Date, String> {
    let year = if u8::from(today.month()) >= 4 {
        today.year()
    } else {
        today.year() - 1
    };
    Date::from_calendar_date(year, Month::April, 1)
        .map_err(|_| crate::i18n::t("setup.notice.date_range"))
}

/// The last day of the financial year that starts on `fy_start` (31 March of the next year).
///
/// # Errors
/// A message when the date is outside the calendar range.
pub fn financial_year_end(fy_start: Date) -> Result<Date, String> {
    Date::from_calendar_date(fy_start.year() + 1, Month::March, 31)
        .map_err(|_| crate::i18n::t("setup.notice.date_range"))
}

/// The three-letter English month names the books date may be written with.
const MONTH_ABBREVIATIONS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The translation keys of the month words, in calendar order.
const MONTH_KEYS: [&str; 12] = [
    "format.month.jan",
    "format.month.feb",
    "format.month.mar",
    "format.month.apr",
    "format.month.may",
    "format.month.jun",
    "format.month.jul",
    "format.month.aug",
    "format.month.sep",
    "format.month.oct",
    "format.month.nov",
    "format.month.dec",
];

/// The month number (1 to 12) a typed month word names: the English abbreviation, or the
/// abbreviation of the current language.
fn month_from_word(word: &str) -> Option<u8> {
    let tr = crate::i18n::current();
    MONTH_ABBREVIATIONS
        .iter()
        .position(|m| m.eq_ignore_ascii_case(word))
        .or_else(|| MONTH_KEYS.iter().position(|k| tr.t(k) == word))
        .and_then(|i| u8::try_from(i + 1).ok())
}

/// Reads the "Books begin from" text: `1 Apr 2026` (as the box shows it) or `YYYY-MM-DD`. Books
/// may begin on any day of the financial year (Tally help, read 2026-11-23: change the date to the
/// actual start of transactions).
///
/// # Errors
/// A message when the text is not a date, or the date is outside `fy_start` to `fy_end`.
pub fn parse_books_begin(text: &str, fy_start: Date, fy_end: Date) -> Result<Date, String> {
    let not_a_date = || crate::i18n::t("setup.notice.type_date");
    let words: Vec<&str> = text.split_whitespace().collect();
    let (year, month, day) = if let [day, month, year] = words.as_slice() {
        let month = month_from_word(month).ok_or_else(not_a_date)?;
        (
            (*year).parse::<i32>().map_err(|_| not_a_date())?,
            month,
            (*day).parse::<u8>().map_err(|_| not_a_date())?,
        )
    } else {
        let parts: Vec<&str> = text.trim().split('-').collect();
        let [year, month, day] = parts.as_slice() else {
            return Err(not_a_date());
        };
        (
            year.parse::<i32>().map_err(|_| not_a_date())?,
            month.parse::<u8>().map_err(|_| not_a_date())?,
            day.parse::<u8>().map_err(|_| not_a_date())?,
        )
    };
    let month = Month::try_from(month).map_err(|_| not_a_date())?;
    let date = Date::from_calendar_date(year, month, day).map_err(|_| not_a_date())?;
    if date < fy_start || date > fy_end {
        return Err(crate::i18n::tf(
            "setup.notice.books_between",
            &[("from", &fy_start.to_string()), ("to", &fy_end.to_string())],
        ));
    }
    Ok(date)
}

/// One feature row of the wizard (index.html:1872-1880).
#[derive(Clone)]
pub struct FeatureRow {
    /// The row's title.
    pub label: String,
    /// The one-line explanation the mock gives.
    pub hint: String,
    /// On by default in the mock (index.html:1872).
    pub default_on: bool,
    /// The model switch this row turns on, when the model has one.
    pub switch: Option<fn(&mut Features) -> &mut bool>,
    /// Why the row cannot be turned on, when the model has no switch for it.
    pub unavailable: Option<String>,
}

/// The feature rows of the wizard. A row is a working switch only when the model has the switch
/// (`ledgerly_model::Features`); the rest say why they are not available.
#[must_use]
pub fn feature_rows() -> Vec<FeatureRow> {
    vec![
        FeatureRow {
            label: crate::i18n::t("setup.feature.inventory.label"),
            hint: crate::i18n::t("setup.feature.inventory.hint"),
            default_on: true,
            switch: Some(|f| &mut f.inventory),
            unavailable: None,
        },
        FeatureRow {
            label: crate::i18n::t("invoice.col.gst"),
            hint: crate::i18n::t("setup.feature.gst.hint"),
            default_on: true,
            switch: Some(|f| &mut f.gst),
            unavailable: None,
        },
        FeatureRow {
            label: crate::i18n::t("sidebar.bank_recon"),
            hint: crate::i18n::t("setup.feature.bank.hint"),
            default_on: true,
            switch: Some(|f| &mut f.bank_reconciliation),
            unavailable: None,
        },
        FeatureRow {
            label: crate::i18n::t("setup.feature.bill_wise.label"),
            hint: crate::i18n::t("setup.feature.bill_wise.hint"),
            default_on: true,
            switch: Some(|f| &mut f.bill_wise),
            unavailable: None,
        },
        FeatureRow {
            label: crate::i18n::t("setup.feature.e_invoice.label"),
            hint: crate::i18n::t("setup.feature.e_invoice.hint"),
            default_on: true,
            switch: None,
            unavailable: Some(crate::i18n::t("setup.feature.e_invoice.reason")),
        },
        FeatureRow {
            label: crate::i18n::t("setup.feature.currencies.label"),
            hint: crate::i18n::t("setup.feature.currencies.hint"),
            default_on: false,
            switch: None,
            unavailable: Some(crate::i18n::t("setup.feature.currencies.reason")),
        },
        FeatureRow {
            label: crate::i18n::t("masters.kind_plural.cost_centre"),
            hint: crate::i18n::t("setup.feature.cost_centres.hint"),
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

/// Builds the company the form describes, with the features chosen. A GSTIN that does not agree
/// with the state is refused, as the form refuses it.
///
/// # Errors
/// A message naming the field that is not acceptable.
pub fn build_company(form: &CompanyForm, features: Features) -> Result<Company, String> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(crate::i18n::t("setup.notice.type_name"));
    }
    let (registrations, pan) = match check_gstin(&form.gstin, form.state) {
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
