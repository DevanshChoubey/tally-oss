//! Card C1 (Wave 3): the shell fetches the books through the worker after open and after every
//! saved command, so screens draw real masters, and masters are created with API-assigned ids.
//!
//! Provenance. `derived`: a new company file holds the 28 reserved groups with ids 1..=28
//! (`ledgerly_model::seed_groups` header) and no ledgers, and the contract (`contracts.md`) says
//! an id of 0 gets the next free id of its kind. Tally, read on 2026-11-21: "Duplicate names are
//! not allowed." in
//! <https://help.tallysolutions.com/docs/te9rel55/Creating_Masters/Accounts_Info/Creating_Ledger_in_Normal_Mode.htm>.
//! The inline Alt+C path of voucher entry is `pending` here (it needs a drawn frame with focus);
//! it uses the same `Command::CreateLedger` with id 0 as this test.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::{Duration, Instant};

use ledgerly_api::{Api, Command, CreatedId};
use ledgerly_app::shell::Shell;
use ledgerly_app::shell::submit::{self, CommandReply};
use ledgerly_app::worker::{ApiWorker, Ticket};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{
    Clock, CompanyId, GroupId, LedgerId, OffsetDateTime, Paise, SignedAmount, StateCode,
};
use ledgerly_model::{
    Address, Company, EntityKind, Features, Group, Ledger, Nature, NegativeStockPolicy,
};
use time::macros::date;

struct Blank;

impl Screen for Blank {
    fn title(&self, _tr: &ledgerly_app::i18n::Tr) -> String {
        "Blank".to_owned()
    }
    fn ui(&mut self, _ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        ScreenAction::Stay
    }
}

fn open_company(name: &str) -> Api {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("c1_books");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.ledgerly"));
    if path.exists() {
        std::fs::remove_file(&path).unwrap();
    }
    let company = Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Asha Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    };
    Api::create(&path, &company, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap()
}

/// A shell holding a worker over a fresh company, as after the welcome screen opened it.
fn shell_with(name: &str) -> Shell {
    let mut shell = Shell::new(Box::new(Blank));
    let worker = ApiWorker::spawn(open_company(name)).unwrap();
    shell.app_ctx_mut().api = Some(worker);
    submit::note_changed();
    shell
}

fn frame(ctx: &egui::Context, shell: &mut Shell) {
    let out = ctx.run_ui(egui::RawInput::default(), |ui| shell.show(ui));
    out.drop_without_applying_deltas();
}

/// Runs frames until `done` holds for the shell's books (bounded).
fn frames_until(shell: &mut Shell, done: impl Fn(&AppCtx) -> bool) {
    let ctx = egui::Context::default();
    let start = Instant::now();
    loop {
        frame(&ctx, shell);
        if done(shell.app_ctx_mut()) {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the books did not arrive in 10 seconds"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn answer(mut ticket: Ticket<CommandReply>) -> CommandReply {
    let start = Instant::now();
    loop {
        if let Some(reply) = submit::settle(&mut ticket) {
            return reply;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "no answer");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn group(name: &str) -> Group {
    Group {
        ineligible_itc: None,
        id: GroupId(0),
        name: name.to_owned(),
        aliases: Vec::new(),
        parent: None,
        reserved: None,
        nature: Some(Nature::Expense),
        affects_gross_profit: false,
        tally: None,
    }
}

fn ledger(name: &str, under: GroupId) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(0),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(under),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn worker_of(shell: &mut Shell) -> &ApiWorker {
    shell.app_ctx_mut().api.as_ref().unwrap()
}

#[test]
fn the_shell_fetches_the_books_after_a_company_is_opened() {
    let mut shell = shell_with("opened");
    frames_until(&mut shell, |c| c.books.is_some());
    // derived: seed has 28 reserved groups, no ledgers.
    let books = shell.app_ctx_mut().books.as_ref().unwrap();
    assert_eq!(books.masters.groups.len(), 28);
    assert_eq!(books.masters.ledgers.len(), 0);
}

#[test]
fn a_created_group_and_ledger_appear_in_the_books_with_their_assigned_ids() {
    let mut shell = shell_with("created");
    frames_until(&mut shell, |c| c.books.is_some());

    let reply = answer(submit::send(
        worker_of(&mut shell),
        Command::CreateGroup(group("Marketing")),
    ));
    let CommandReply::Booked(result) = reply else {
        panic!("expected booked, got {reply:?}");
    };
    // derived: ids 1..=28 are the seed, so the first free group id is 29.
    assert_eq!(result.created, Some(CreatedId::Group(GroupId(29))));

    let reply = answer(submit::send(
        worker_of(&mut shell),
        Command::CreateLedger(ledger("Ads", GroupId(29))),
    ));
    let CommandReply::Booked(result) = reply else {
        panic!("expected booked, got {reply:?}");
    };
    // derived: no ledgers existed, so the first id is 1.
    assert_eq!(result.created, Some(CreatedId::Ledger(LedgerId(1))));

    frames_until(&mut shell, |c| {
        c.books
            .as_ref()
            .is_some_and(|b| b.masters.ledgers.contains_key(&LedgerId(1)))
    });
    let books = shell.app_ctx_mut().books.as_ref().unwrap();
    assert_eq!(books.masters.groups[&GroupId(29)].name, "Marketing");
    assert_eq!(books.masters.ledgers[&LedgerId(1)].name, "Ads");
    assert_eq!(books.masters.ledgers[&LedgerId(1)].group, Some(GroupId(29)));
}

#[test]
fn a_duplicate_name_is_refused_as_failed_and_the_books_keep_one_copy() {
    let mut shell = shell_with("duplicate");
    frames_until(&mut shell, |c| c.books.is_some());
    let first = answer(submit::send(
        worker_of(&mut shell),
        Command::CreateGroup(group("Marketing")),
    ));
    assert!(matches!(first, CommandReply::Booked(_)), "got {first:?}");
    // Tally: "Duplicate names are not allowed." (page cited in the header).
    let second = answer(submit::send(
        worker_of(&mut shell),
        Command::CreateGroup(group("Marketing")),
    ));
    assert!(matches!(second, CommandReply::Failed(_)), "got {second:?}");
    frames_until(&mut shell, |c| {
        c.books
            .as_ref()
            .is_some_and(|b| b.masters.groups.contains_key(&GroupId(29)))
    });
    let books = shell.app_ctx_mut().books.as_ref().unwrap();
    let copies = books
        .masters
        .groups
        .values()
        .filter(|g| g.name == "Marketing")
        .count();
    assert_eq!(copies, 1);
}
