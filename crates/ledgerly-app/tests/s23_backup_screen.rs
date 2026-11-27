//! Round s23: the app's open path (upgrade of an old file, after a backup) and the Backups and
//! updates screen (mock states `backup.*`, `design/mock-v2/index.html`).
//!
//! Provenance (`derived`):
//! - Behaviour: TallyPrime help, "Local Backup" (<https://help.tallysolutions.com/local-backup/>,
//!   read 2026-11-27: a backup path, frequency Daily by default, 7 restorable versions, 1 to 366,
//!   backups never overwrite one another) and "Restore Backup Data"
//!   (<https://help.tallysolutions.com/restore-backup-data/>, read 2026-11-27: versions listed by
//!   date and time, newest first). The predefined ledgers Cash and Profit & Loss A/c come from
//!   "Ledgers in TallyPrime" (<https://help.tallysolutions.com/ledgers-in-tallyprime/>, read
//!   2026-11-27); the fixture `pre-s22.ledgerly` was written by a build before round s22 and has
//!   neither (checked in `ledgerly-api/tests/s23_upgrade.rs`).
//! - The words are the screen's English strings, which are the mock's (`SCREENS.backup`).
//! - Every path, setting, release tag and hash is one the test wrote or served itself.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::epaint::{ClippedShape, Shape};
use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::update_check::{UpdateCheck, UpdateChecker};
use ledgerly_api::{
    Api, BackupFrequency, BackupSettings, Company, EntityKind, Features, NegativeStockPolicy,
    PredefinedStep,
};
use ledgerly_app::screens::backup::{BackupScreen, Tab};
use ledgerly_app::worker::{ApiWorker, Poll};
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Clock, CompanyId, Outcome, StateCode};
use ledgerly_model::{Address, ReservedLedger};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::macros::date;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ledgerly-api/tests/fixtures/pre-s22.ledgerly"
);

fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("s23-app-{name}"));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

/// A new company in its own folder, and the app context holding it.
fn app_with_company(name: &str) -> (PathBuf, AppCtx) {
    let path = dir(name).join("fictional.ledgerly");
    let api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let app = AppCtx {
        api: Some(ApiWorker::spawn(api).unwrap()),
        ..AppCtx::default()
    };
    (path, app)
}

fn drawn_text(shapes: &[ClippedShape]) -> String {
    fn walk(shape: &Shape, out: &mut String) {
        if let Shape::Text(text) = shape {
            out.push_str(text.galley.text());
            out.push('\n');
        } else if let Shape::Vec(shapes) = shape {
            for shape in shapes {
                walk(shape, out);
            }
        }
    }
    let mut out = String::new();
    for clipped in shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

fn frame(screen: &mut BackupScreen, app: &mut AppCtx, keys: &[(Key, Modifiers)]) -> String {
    let ctx = egui::Context::default();
    let events = keys
        .iter()
        .map(|&(key, modifiers)| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        })
        .collect();
    let out = ctx.run_ui(
        RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 1600.0),
            )),
            ..RawInput::default()
        },
        |ui| {
            let _stay = screen.ui(ui, app);
        },
    );
    let text = drawn_text(&out.shapes);
    out.drop_without_applying_deltas();
    text
}

/// Frames until the screen is idle and has drawn `wanted`.
fn settle_on(screen: &mut BackupScreen, app: &mut AppCtx, wanted: &str) -> String {
    let mut text = frame(screen, app, &[]);
    for _ in 0..3000 {
        if screen.loaded() && !screen.busy() && text.contains(wanted) {
            return text;
        }
        std::thread::sleep(Duration::from_millis(2));
        text = frame(screen, app, &[]);
    }
    panic!("never drew {wanted:?}; last frame:\n{text}");
}

/// Runs `job` on the app's worker and waits for its answer.
fn on_worker<R: Send + 'static>(
    app: &AppCtx,
    job: impl FnOnce(&mut Api) -> R + Send + 'static,
) -> R {
    let mut ticket = app.api.as_ref().unwrap().run(job);
    for _ in 0..5000 {
        match ticket.poll() {
            Poll::Ready(r) => return r,
            Poll::Pending => std::thread::sleep(Duration::from_millis(1)),
            Poll::Taken | Poll::Lost => panic!("the worker stopped"),
        }
    }
    panic!("the worker never answered");
}

fn files_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<_> = entries.map(|e| e.unwrap().path()).collect();
    v.sort();
    v
}

const ALT: Modifiers = Modifiers::ALT;
const NONE: Modifiers = Modifiers::NONE;
const CTRL: Modifiers = Modifiers::COMMAND;

// ---- the open path ---------------------------------------------------------------------------

#[test]
fn opening_an_old_file_from_the_app_backs_it_up_upgrades_it_and_leaves_a_note() {
    let folder = dir("open-old");
    let path = folder.join("old-traders.ledgerly");
    std::fs::copy(FIXTURE, &path).unwrap();
    // Start, the welcome form and Company setup's Open all go through this call.
    let api = ledgerly_app::recent::open_and_record(&path, None).unwrap();
    let note = ledgerly_app::open_company::take_pending().expect("an upgrade note");
    let upgrade = note.upgrade.clone().expect("the file was upgraded");
    assert!(note.not_upgraded.is_none(), "{note:?}");
    // The backup was taken first, into Backups beside the file, and is a checked copy.
    let backups = files_in(&folder.join("Backups"));
    assert_eq!(backups, vec![upgrade.backup.clone()]);
    assert!(
        Api::open(&upgrade.backup)
            .unwrap()
            .verify()
            .unwrap()
            .is_ok()
    );
    // Derived from the fixture (s23_upgrade.rs): its hand-made Cash sits in Cash-in-Hand, so it
    // takes the role; Profit & Loss A/c is created.
    assert!(
        matches!(
            upgrade.ledgers[..],
            [
                PredefinedStep::Adopted {
                    role: ReservedLedger::Cash,
                    ..
                },
                PredefinedStep::Created {
                    role: ReservedLedger::ProfitAndLoss,
                    ..
                }
            ]
        ),
        "{:?}",
        upgrade.ledgers
    );
    let Outcome::Value(books) = api.books().unwrap() else {
        panic!("books")
    };
    for role in [ReservedLedger::Cash, ReservedLedger::ProfitAndLoss] {
        assert!(
            books
                .masters
                .ledgers
                .values()
                .any(|l| l.reserved == Some(role)),
            "{role:?} missing"
        );
    }
    // The note is taken once.
    assert!(ledgerly_app::open_company::take_pending().is_none());

    // The home screen pushes the Backups screen with the note; it says what happened.
    let mut app = AppCtx {
        api: Some(ApiWorker::spawn(api).unwrap()),
        ..AppCtx::default()
    };
    let mut screen = BackupScreen::after_open(note);
    let text = settle_on(&mut screen, &mut app, "Backups in this folder");
    assert!(text.contains("has been upgraded"), "{text}");
    assert!(text.contains("A backup was taken first:"), "{text}");
    assert!(
        text.contains("Cash: your ledger Cash is now the predefined Cash ledger."),
        "{text}"
    );
    assert!(text.contains("Profit & Loss A/c: created."), "{text}");
    assert!(text.contains("The change is in the audit trail"), "{text}");
}

#[test]
fn an_old_file_whose_backup_cannot_be_written_opens_as_it_was_and_says_why() {
    let folder = dir("open-no-backup");
    let path = folder.join("old-traders.ledgerly");
    std::fs::copy(FIXTURE, &path).unwrap();
    // A backup folder inside a regular file cannot be made.
    let blocker = folder.join("a-file");
    std::fs::write(&blocker, b"not a folder").unwrap();
    let mut api = Api::open(&path).unwrap();
    api.set_backup_settings(&BackupSettings {
        folder: Some(blocker.join("Backups")),
        frequency: BackupFrequency::Off,
        keep: 7,
    })
    .unwrap();
    drop(api);
    let (api, note) = ledgerly_app::open_company::open_company(&path).unwrap();
    let note = note.expect("a note");
    assert!(note.upgrade.is_none(), "{note:?}");
    assert!(note.not_upgraded.is_some(), "{note:?}");
    let Outcome::Value(books) = api.books().unwrap() else {
        panic!("books")
    };
    assert!(
        !books
            .masters
            .ledgers
            .values()
            .any(|l| l.reserved == Some(ReservedLedger::ProfitAndLoss)),
        "nothing is written without the backup"
    );
}

#[test]
fn opening_a_company_whose_daily_backup_is_due_takes_it() {
    let folder = dir("open-due");
    let path = folder.join("fictional.ledgerly");
    // create_company adds the predefined ledgers, so this file needs no upgrade.
    let Outcome::Value(api) = Api::create_company(&path, &company(), Clock::System).unwrap() else {
        panic!("created")
    };
    drop(api);
    let mut api = Api::open(&path).unwrap();
    api.set_backup_settings(&BackupSettings {
        folder: None,
        frequency: BackupFrequency::Daily,
        keep: 7,
    })
    .unwrap();
    drop(api);
    let (_api, _note) = ledgerly_app::open_company::open_company(&path).unwrap();
    let written = files_in(&folder.join("Backups"));
    assert!(
        written.iter().any(|p| p
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("fictional-scheduled-")),
        "{written:?}"
    );
}

// ---- the Backup and restore tab --------------------------------------------------------------

#[test]
fn the_backup_tab_shows_the_settings_and_an_empty_folder() {
    let (path, mut app) = app_with_company("tab-empty");
    let mut screen = BackupScreen::with_update_check(Tab::Backup, None, UpdateChecker::github());
    let text = settle_on(&mut screen, &mut app, "Backup settings");
    for words in [
        "Backup folder",
        "Scheduled backup",
        "Off",
        "Daily",
        "Weekly",
        "Monthly",
        "Restorable versions",
        "Save settings",
        "Back up now",
        "Backups in this folder",
        "No backups in this folder yet.",
    ] {
        assert!(text.contains(words), "missing {words:?}:\n{text}");
    }
    let beside = path.parent().unwrap().join("Backups").display().to_string();
    assert!(
        text.contains(&beside),
        "the default folder is named:\n{text}"
    );
}

#[test]
fn saving_stores_the_folder_frequency_and_versions_and_refuses_a_bad_count() {
    let (path, mut app) = app_with_company("tab-save");
    let mut screen = BackupScreen::with_update_check(Tab::Backup, None, UpdateChecker::github());
    let _ = settle_on(&mut screen, &mut app, "Backup settings");
    let chosen = path.parent().unwrap().join("chosen");
    screen.set_folder(chosen.display().to_string());
    screen.set_frequency(BackupFrequency::Weekly);
    screen.set_versions("0");
    let _ = frame(&mut screen, &mut app, &[(Key::A, CTRL)]);
    let text = settle_on(
        &mut screen,
        &mut app,
        "must be a whole number from 1 to 366",
    );
    assert!(!text.contains("Backup settings saved."), "{text}");
    let stored = on_worker(&app, |api| api.backup_settings().unwrap());
    assert_eq!(stored.frequency, BackupFrequency::Off, "nothing was sent");

    screen.set_versions("30");
    let _ = frame(&mut screen, &mut app, &[(Key::A, CTRL)]);
    let _ = settle_on(&mut screen, &mut app, "Backup settings saved.");
    let stored = on_worker(&app, |api| api.backup_settings().unwrap());
    assert_eq!(
        stored,
        BackupSettings {
            folder: Some(chosen),
            frequency: BackupFrequency::Weekly,
            keep: 30,
        }
    );
}

#[test]
fn alt_b_backs_up_now_and_the_list_shows_it() {
    let (path, mut app) = app_with_company("tab-now");
    let mut screen = BackupScreen::with_update_check(Tab::Backup, None, UpdateChecker::github());
    let _ = settle_on(&mut screen, &mut app, "Backup settings");
    let _ = frame(&mut screen, &mut app, &[(Key::B, ALT)]);
    let text = settle_on(&mut screen, &mut app, "Backup taken and checked.");
    let written = files_in(&path.parent().unwrap().join("Backups"));
    assert_eq!(written.len(), 1, "{written:?}");
    let name = written[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(text.contains(&name), "the list shows the file:\n{text}");
    assert!(text.contains("Manual"), "{text}");
    assert_eq!(screen.backups().len(), 1);
}

#[test]
fn enter_then_ctrl_a_restores_the_selected_backup_to_a_new_checked_file() {
    let (path, mut app) = app_with_company("tab-restore");
    let mut screen = BackupScreen::with_update_check(Tab::Backup, None, UpdateChecker::github());
    let _ = settle_on(&mut screen, &mut app, "Backup settings");
    let _ = frame(&mut screen, &mut app, &[(Key::B, ALT)]);
    let _ = settle_on(&mut screen, &mut app, "Backup taken and checked.");
    let text = frame(&mut screen, &mut app, &[(Key::Enter, NONE)]);
    let text = if text.contains("Restore to") {
        text
    } else {
        frame(&mut screen, &mut app, &[])
    };
    assert!(text.contains("Restore to"), "{text}");
    assert!(
        text.contains("It never replaces a file that exists."),
        "{text}"
    );
    let target = path.with_file_name("fictional-restored.ledgerly");
    assert!(text.contains(&target.display().to_string()), "{text}");
    let _ = frame(&mut screen, &mut app, &[(Key::A, CTRL)]);
    let text = settle_on(&mut screen, &mut app, "Restored to");
    assert!(text.contains("Audit chain verified"), "{text}");
    assert!(Api::open(&target).unwrap().verify().unwrap().is_ok());
}

#[test]
fn a_backup_with_a_broken_chain_is_refused_and_nothing_is_written() {
    let (path, mut app) = app_with_company("tab-refused");
    let mut screen = BackupScreen::with_update_check(Tab::Backup, None, UpdateChecker::github());
    let _ = settle_on(&mut screen, &mut app, "Backup settings");
    let _ = frame(&mut screen, &mut app, &[(Key::B, ALT)]);
    let _ = settle_on(&mut screen, &mut app, "Backup taken and checked.");
    let backup = screen.backups()[0].path.clone();
    let conn = rusqlite::Connection::open(&backup).unwrap();
    let mut event: Vec<u8> = conn
        .query_row("SELECT event FROM events WHERE seq = 1", [], |r| r.get(0))
        .unwrap();
    event[0] ^= 0x01;
    conn.execute("UPDATE events SET event = ?1 WHERE seq = 1", [event])
        .unwrap();
    drop(conn);
    let target = path.with_file_name("from-broken.ledgerly");
    screen.start_restore();
    screen.set_restore_to(target.display().to_string());
    let _ = frame(&mut screen, &mut app, &[(Key::A, CTRL)]);
    let text = settle_on(&mut screen, &mut app, "Not restored.");
    assert!(text.contains("broken at record 1"), "{text}");
    assert!(!target.exists());
}

// ---- the Updates tab -------------------------------------------------------------------------

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

/// A server that plays the releases API with `tag` as the latest release, and counts requests.
fn release_server(tag: &str) -> (String, Arc<Mutex<usize>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let setup = b"pretend installer bytes".to_vec();
    let sums = format!("{}  ledgerly-setup.exe\n", hex(&setup));
    let json = format!(
        r#"{{"tag_name":"{tag}","html_url":"{origin}/releases/tag/{tag}","assets":[
 {{"name":"ledgerly-setup.exe","digest":"sha256:{i}","browser_download_url":"{origin}/download/setup.exe"}},
 {{"name":"SHA256SUMS.txt","digest":"sha256:{s}","browser_download_url":"{origin}/download/sums"}}]}}"#,
        i = hex(&setup),
        s = hex(sums.as_bytes()),
    );
    let routes = vec![
        ("/latest".to_owned(), json.into_bytes()),
        ("/download/sums".to_owned(), sums.into_bytes()),
    ];
    let count = Arc::new(Mutex::new(0_usize));
    let seen = Arc::clone(&count);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            *seen.lock().unwrap() += 1;
            reply(stream, &routes);
        }
    });
    (origin, count)
}

fn reply(mut stream: TcpStream, routes: &[(String, Vec<u8>)]) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut first = String::new();
    let _ = reader.read_line(&mut first);
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
    }
    let path = first.split_whitespace().nth(1).unwrap_or("").to_owned();
    let (code, body) = routes
        .iter()
        .find(|(p, _)| *p == path)
        .map_or((404, b"{}".to_vec()), |(_, b)| (200, b.clone()));
    let head = format!(
        "HTTP/1.1 {code} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
    let mut rest = Vec::new();
    let _ = reader.read_to_end(&mut rest);
}

fn settle_check(screen: &mut BackupScreen, app: &mut AppCtx) -> String {
    let mut text = frame(screen, app, &[]);
    for _ in 0..3000 {
        if screen.update_result().is_some() && !screen.busy() {
            return frame(screen, app, &[]);
        }
        std::thread::sleep(Duration::from_millis(2));
        text = frame(screen, app, &[]);
    }
    panic!("the check never finished; last frame:\n{text}");
}

#[test]
fn the_update_check_is_off_by_default_and_sends_nothing() {
    let (origin, count) = release_server("v99.0.0");
    let prefs = dir("updates-off");
    let checker = UpdateChecker::with_endpoints(&format!("{origin}/latest"), &[origin.as_str()]);
    let mut screen = BackupScreen::with_update_check(Tab::Updates, Some(prefs), checker);
    let mut app = AppCtx::default();
    let text = frame(&mut screen, &mut app, &[]);
    assert!(
        text.contains("Let Ledgerly check for updates when I ask"),
        "{text}"
    );
    assert!(text.contains("Turn this on to check."), "{text}");
    let _ = frame(&mut screen, &mut app, &[(Key::U, NONE)]);
    std::thread::sleep(Duration::from_millis(50));
    let _ = frame(&mut screen, &mut app, &[]);
    assert!(screen.update_result().is_none());
    assert_eq!(*count.lock().unwrap(), 0, "no request while off");
}

#[test]
fn turned_on_the_check_shows_a_newer_release_with_full_hashes_and_its_page() {
    let (origin, count) = release_server("v99.0.0");
    let prefs = dir("updates-on");
    let checker = UpdateChecker::with_endpoints(&format!("{origin}/latest"), &[origin.as_str()]);
    let mut screen = BackupScreen::with_update_check(Tab::Updates, Some(prefs.clone()), checker);
    let mut app = AppCtx::default();
    screen.set_update_on(true);
    // The choice is kept for the next start.
    assert_eq!(ledgerly_app::update_pref::load(&prefs), Ok(true));
    let _ = frame(&mut screen, &mut app, &[(Key::U, NONE)]);
    let text = settle_check(&mut screen, &mut app);
    assert!(
        matches!(screen.update_result(), Some(UpdateCheck::Available(_))),
        "{:?}",
        screen.update_result()
    );
    assert!(text.contains("Ledgerly 99.0.0 is available"), "{text}");
    assert!(text.contains("ledgerly-setup.exe"), "{text}");
    assert!(
        text.contains(&hex(b"pretend installer bytes")),
        "the full hash:\n{text}"
    );
    assert!(
        text.contains(&format!("Release page: {origin}/releases/tag/v99.0.0")),
        "{text}"
    );
    assert!(text.contains("Ledgerly does not install it."), "{text}");
    assert!(*count.lock().unwrap() >= 2, "the release and its sums file");
}

#[test]
fn a_check_that_cannot_reach_the_server_says_so() {
    // A port that was bound and released has nothing listening.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let origin = format!("http://127.0.0.1:{port}");
    let checker = UpdateChecker::with_endpoints(&format!("{origin}/latest"), &[origin.as_str()]);
    let mut screen =
        BackupScreen::with_update_check(Tab::Updates, Some(dir("updates-down")), checker);
    let mut app = AppCtx::default();
    screen.set_update_on(true);
    let _ = frame(&mut screen, &mut app, &[(Key::U, NONE)]);
    let text = settle_check(&mut screen, &mut app);
    assert!(text.contains("Could not check for updates."), "{text}");
}
