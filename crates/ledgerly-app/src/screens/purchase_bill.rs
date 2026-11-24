//! Purchase from a photo of a bill (mock `SCREENS['purchase-bill']`, index.html:1420-1458; state
//! `purchase-bill.offline-ai`).
//!
//! Spec 6.8: a bill image or PDF becomes a `VoucherDraft` whose figures are transcriptions with
//! their source and confidence; the engine recomputes the totals and a mismatch blocks Accept.
//! `ledgerly_assist::ocr::read_bill` reads OCR text lines into such a draft, but no OCR engine
//! in this build turns an image into those lines, so no bill can be read. The screen keeps the
//! mock's two columns: on the left the place of the bill image, which says no image is read or
//! kept; on the right the note for the company's stored tier ([`crate::screens::ai_gate`]) and
//! the way to type the bill instead, the purchase form (F9). No reading, draft or figure is drawn.
//!
//! The mock's other states (reading, proposal, low confidence, mismatch, duplicate, cloud consent,
//! unreadable file, accepted) need the reader and are listed in `design/PARITY-GAPS.md`.

use crate::i18n::Tr;
use egui::{Frame, Key, Margin, Modifiers, Ui, vec2};

use crate::keymap::Action;
use crate::screen::KeyHint;
use crate::screens::ai_gate::{self, Feature, Gate, TierGate};
use crate::theme::RADIUS_M;
use crate::ui::button::{ButtonKind, button};
use crate::ui::empty::empty_state;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::tokens;
use crate::{AppCtx, Screen, ScreenAction};

/// The screen's title in the palette, the sidebar and the breadcrumb (index.html:1420).
pub const TITLE: &str = "Purchase from bill photo";
/// The page head's title (index.html:1422).
pub const PAGE_TITLE: &str = "Purchase from a bill photo";
/// The page head's purpose line: the first part of the mock's (index.html:1422). Its "Alt+O read
/// another bill" and "the image is kept with the voucher" need the reader and an image store.
pub const SUBTITLE: &str = "F9 purchase";
/// The image column's title when no bill is read.
pub const NO_IMAGE_TITLE: &str = "No bill photo";
/// The image column's sentence when no bill is read.
pub const NO_IMAGE_SENTENCE: &str = "This build reads no bill, so no image is chosen or kept.";

/// The sidebar entry selected on this screen: Purchase (the sidebar's label, `navigate::sidebar`).
pub const SIDEBAR_TITLE: &str = "Purchase";

/// The key bar's name for the screen. The mock names its key bar by the draft ("Draft purchase",
/// index.html:1455); there is no draft here, so it names the screen.
pub const KEY_BAR_NAME: &str = "Bill photo";

/// The key bar entries: the mock's proposal key bar starts with the screen's state name; here it
/// is "Bill photo", then F9 for the purchase form. The mock's draft keys (index.html:1455) act on
/// a draft, which never exists here. Esc is the shell's.
#[must_use]
pub fn key_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("", "Bill photo"),
        KeyHint::new("F9", "Purchase"),
    ]
}

/// The Purchase from bill photo screen.
#[derive(Debug, Default)]
pub struct PurchaseBillScreen {
    /// The company's AI tier, read through the worker.
    gate: TierGate,
    /// What the screen knows of the tier, as of the last frame.
    last: Option<Gate>,
    /// Why the last button or key did not open its screen.
    notice: Option<String>,
}

impl PurchaseBillScreen {
    /// Whether the tier has been read (or cannot be: no company, or the read failed).
    #[must_use]
    pub fn loaded(&self) -> bool {
        match &self.last {
            None | Some(Gate::Reading) => false,
            Some(
                Gate::NoCompany
                | Gate::Failed(_)
                | Gate::Unset
                | Gate::Off
                | Gate::Local
                | Gate::Cloud,
            ) => true,
        }
    }

    /// Turns a request to open a screen into the stack's action, keeping the reason when it
    /// does not open.
    fn opened(&mut self, result: Result<ScreenAction, String>) -> ScreenAction {
        match result {
            Ok(action) => action,
            Err(why) => {
                self.notice = Some(why);
                ScreenAction::Stay
            }
        }
    }
}

/// The image column's width inside its 18-point padding: 400 points in all (index.html:1432:
/// `grid-template-columns:400px ...`), less 2 x 18.
const IMAGE_INNER_WIDTH: f32 = 364.0;
/// The image column's least height inside its padding: 520 points in all (`.billwrap`,
/// index.html:296: `min-height:520px`), less 2 x 18.
const IMAGE_INNER_HEIGHT: f32 = 484.0;
/// The space above the empty state, so it sits near the middle of the column.
const IMAGE_TOP: f32 = 180.0;
/// The gap between the columns (`.grid`).
const COLUMN_GAP: f32 = 16.0;

/// What the right column asked for.
enum Ask {
    /// Nothing.
    Nothing,
    /// Open the purchase form.
    Purchase,
    /// Open AI and privacy.
    Settings,
}

/// The image column: a sunken panel (`.billwrap`) saying no bill is read.
fn draw_image(ui: &mut Ui) {
    let t = tokens(ui);
    Frame::new()
        .fill(t.sunken)
        .corner_radius(RADIUS_M)
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.set_min_size(vec2(IMAGE_INNER_WIDTH, IMAGE_INNER_HEIGHT));
            ui.set_max_width(IMAGE_INNER_WIDTH);
            ui.add_space(IMAGE_TOP);
            let _no_action = empty_state(ui, NO_IMAGE_TITLE, NO_IMAGE_SENTENCE, None);
        });
}

/// The gate note's words for `gate` in the language of `tr`: the shared wording of
/// [`ai_gate::reason_in`] for this screen's feature.
#[must_use]
pub fn gate_line_in(gate: &Gate, tr: &Tr) -> Option<String> {
    ai_gate::reason_in(Feature::BillReading, gate, tr)
}

/// The right column: the note with its AI and privacy button, then the purchase form.
fn draw_right(ui: &mut Ui, gate: &Gate, notice: Option<&str>, tr: &Tr) -> Ask {
    let mut asked = Ask::Nothing;
    if ai_gate::gate_note_in(ui, Feature::BillReading, gate, tr) {
        asked = Ask::Settings;
    }
    if let Some(line) = notice {
        ui.add_space(8.0);
        note(ui, NoteKind::Warn, line);
    }
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if button(ui, "Purchase", Some("F9"), ButtonKind::Primary).clicked() {
            asked = Ask::Purchase;
        }
    });
    asked
}

impl Screen for PurchaseBillScreen {
    fn title(&self, _tr: &Tr) -> String {
        TITLE.to_owned()
    }

    fn page_title(&self, _tr: &Tr) -> String {
        PAGE_TITLE.to_owned()
    }

    /// The sidebar keeps Purchase selected, as the mock does (index.html:1422: the crumbs run
    /// Home, Purchase, From bill photo).
    fn sidebar_title(&self, _tr: &Tr) -> String {
        SIDEBAR_TITLE.to_owned()
    }

    fn key_bar_name(&self, _tr: &Tr) -> String {
        KEY_BAR_NAME.to_owned()
    }

    fn subtitle(&self, _tr: &Tr) -> Option<String> {
        Some(SUBTITLE.to_owned())
    }

    fn keys(&self, _tr: &Tr) -> Vec<KeyHint> {
        key_hints()
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        let gate = self.gate.gate(ui, ctx);
        self.last = Some(gate.clone());
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F9)) {
            return self.opened(ai_gate::open_voucher(Action::NewPurchase, ctx));
        }

        page_head(ui, PAGE_TITLE, Some(SUBTITLE), |_| {});
        let notice = self.notice.clone();
        let mut asked = Ask::Nothing;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = COLUMN_GAP;
            draw_image(ui);
            ui.vertical(|ui| asked = draw_right(ui, &gate, notice.as_deref(), &ctx.tr));
        });
        match asked {
            Ask::Nothing => ScreenAction::Stay,
            Ask::Purchase => self.opened(ai_gate::open_voucher(Action::NewPurchase, ctx)),
            Ask::Settings => self.opened(ai_gate::open_settings(ctx)),
        }
    }
}
