//! A drill keeps the sidebar entry of the screen it started from: the mock's group, ledger and
//! voucher states are states of the `balance-sheet` screen, so Balance sheet stays selected
//! (`design/mock-v2/index.html:1594, 1600`).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and fail loudly"
)]

use ledgerly_app::screens::report_view::{
    DrillTarget, ReportSource, ReportToken, ReportViewScreen, ViewCell, ViewDetail, ViewError,
    ViewKey, ViewParams, ViewReport, ViewRow, ViewRowKind, VoucherViewScreen,
};
use ledgerly_app::shell::sidebar;
use ledgerly_app::{Screen, ScreenAction};
use ledgerly_core::{GroupId, Outcome, Period, VoucherId};
use time::macros::date;

fn report(title: &str) -> ViewReport {
    ViewReport {
        token: ReportToken(title.to_owned()),
        title: title.to_owned(),
        params: ViewParams {
            period: Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 20)).unwrap(),
            detail: ViewDetail::Condensed,
        },
        columns: vec!["Amount".to_owned()],
        rows: vec![ViewRow {
            key: Some(ViewKey::Group(GroupId(7))),
            label: "Sundry Debtors".to_owned(),
            depth: 0,
            kind: ViewRowKind::Group,
            cells: vec![ViewCell::Text("x".to_owned())],
        }],
        totals: Vec::new(),
        notes: Vec::new(),
    }
}

fn source() -> ReportSource {
    ReportSource::new(
        |_: &ReportToken, _: ViewParams| -> Result<Outcome<ViewReport>, ViewError> {
            Err(ViewError::Source {
                message: "not used in this test".to_owned(),
            })
        },
        |_: &ReportToken, _: &ViewKey, _: ViewParams| {
            Ok(Outcome::Value(DrillTarget::Report(report(
                "Group Summary: Sundry Debtors",
            ))))
        },
    )
}

fn drill(screen: &mut ReportViewScreen) -> Box<dyn Screen> {
    match screen.drill() {
        ScreenAction::Push(next) => next,
        ScreenAction::Stay | ScreenAction::Replace(_) | ScreenAction::Pop => {
            panic!("the drill did not open a screen: {:?}", screen.notice())
        }
    }
}

#[test]
fn a_drill_keeps_the_sidebar_entry_it_started_from() {
    // provenance: index.html:1594 and 1600: the group and ledger states keep Balance sheet
    // selected. A drill two levels down still names the first screen. Catches the selection
    // following the drilled report's own title ("Group Summary: ..."), which names no entry.
    let mut top = ReportViewScreen::new(report("Balance Sheet"), source()).unwrap();
    let first = drill(&mut top);
    assert_eq!(
        first.title(&ledgerly_app::i18n::Tr::default()),
        "Group Summary: Sundry Debtors"
    );
    assert_eq!(
        first.sidebar_title(&ledgerly_app::i18n::Tr::default()),
        "Balance Sheet"
    );
    assert_eq!(
        sidebar::selected_label(&first.sidebar_title(&ledgerly_app::i18n::Tr::default())),
        Some("Balance sheet")
    );
    // A screen that is not a drill selects by its own title.
    assert_eq!(
        top.sidebar_title(&ledgerly_app::i18n::Tr::default()),
        "Balance Sheet"
    );
}

#[test]
fn a_voucher_opened_from_a_drill_keeps_its_origin() {
    // provenance: index.html:1568: `voucher` is a state of the balance-sheet screen (`BS_LEVEL`).
    let view = VoucherViewScreen::new(VoucherId(3)).with_origin("Balance Sheet");
    assert_eq!(
        view.sidebar_title(&ledgerly_app::i18n::Tr::default()),
        "Balance Sheet"
    );
    assert_eq!(
        VoucherViewScreen::new(VoucherId(3)).sidebar_title(&ledgerly_app::i18n::Tr::default()),
        "Voucher #3"
    );
}
