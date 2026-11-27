//! Round s21: the OCR reader's state and its model download on AI and privacy (mock states
//! `ai.ai-settings.local`, `.local-downloading`, `.local-ready`, `.local-download-failed`,
//! `design/mock-v2/index.html` with `ai=on`).
//!
//! The models folder, the reader and the fetcher come from the app's `OcrSetup`; these tests hand
//! the screen a recorded reader and a recording fetcher over a folder under the test target directory, never
//! the user's folders. Nothing is downloaded: the fetcher answers with fictional bytes.
//!
//! Provenance (derived). The words are the mock's (`SCREENS['ai-settings']`, the S21 OCR models
//! block). The fixed URLs and checksums are `OCR_MODEL_FILES`, observed by the coordinator; the
//! fictional bytes' SHA-256 (2775dbaf...462a) was computed with `sha256sum` for the mock's failed
//! state. A refused file leaves neither the model file nor its side file behind
//! (`download_model`'s contract).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::epaint::{ClippedShape, Shape};
use egui::{Event, PointerButton, Pos2, RawInput, Rect};
use ledgerly_api::{
    AiTier, Api, AssistError, BillOcr, ModelFetch, OCR_MODEL_FILES, OcrOutput, ProviderInfo,
    ProviderPlace, ProviderStatus,
};
use ledgerly_app::ocr_setup::OcrSetup;
use ledgerly_app::screens::ai_settings::AiSettingsScreen;
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{CancelToken, Clock, CompanyId, StateCode};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use time::OffsetDateTime;
use time::macros::date;

const FICTIONAL: &[u8] = b"fictional bytes, not the model";
const MISSING: &str =
    "Not installed: the OCR model files are not there: text-detection.onnx, text-recognition.onnx.";
const DOWNLOAD: &str = "Download the OCR models";
const SOURCE: &str = "Two files, about 12 MB, from ocrs-models.s3-accelerate.amazonaws.com. Each is kept only if its checksum matches.";
const NETWORK: &str = "Bills and books are never sent. The model download, when you press it, is the only connection.";

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

fn folder(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A reader whose status is fixed; it reads nothing here.
#[derive(Debug)]
struct Recorded(ProviderStatus);

impl BillOcr for Recorded {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "ocrs".to_owned(),
            version: "0.14.0".to_owned(),
            place: ProviderPlace::ThisComputer,
        }
    }
    fn status(&self) -> ProviderStatus {
        self.0.clone()
    }
    fn read_png(&self, _: &[u8], _: &str, _: &CancelToken) -> Result<OcrOutput, AssistError> {
        panic!("nothing is read on this screen")
    }
}

fn missing() -> ProviderStatus {
    ProviderStatus::NotInstalled {
        reason: "the OCR model files are not there: text-detection.onnx, text-recognition.onnx"
            .to_owned(),
    }
}

/// Records each URL asked for; answers with the fictional bytes, after `gate` lets it when set.
#[derive(Debug, Default)]
struct Recording {
    urls: Mutex<Vec<String>>,
    gate: Mutex<Option<Receiver<()>>>,
}

impl ModelFetch for Recording {
    fn get(&self, url: &str) -> Result<Vec<u8>, AssistError> {
        self.urls.lock().unwrap().push(url.to_owned());
        if let Some(gate) = self.gate.lock().unwrap().as_ref() {
            let _let_through = gate.recv_timeout(Duration::from_secs(30));
        }
        Ok(FICTIONAL.to_vec())
    }
}

impl Recording {
    fn urls(&self) -> Vec<String> {
        self.urls.lock().unwrap().clone()
    }
}

struct Harness {
    ctx: egui::Context,
    screen: AiSettingsScreen,
    app: AppCtx,
    shapes: Vec<ClippedShape>,
}

fn texts(shapes: &[ClippedShape]) -> Vec<(String, Rect)> {
    fn walk(shape: &Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            Shape::Text(t) => out.push((t.galley.text().to_owned(), shape.visual_bounding_rect())),
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            Shape::Noop
            | Shape::Circle(_)
            | Shape::Ellipse(_)
            | Shape::LineSegment { .. }
            | Shape::Path(_)
            | Shape::Rect(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Band(_)
            | Shape::Callback(_) => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

impl Harness {
    fn new(name: &str, reader: ProviderStatus, dir: &Path, fetch: Arc<Recording>) -> Self {
        ledgerly_app::release::set_ai_enabled(true);
        let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
        let _removed = std::fs::remove_file(&path);
        let mut api =
            Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
        api.set_ai_tier(AiTier::Local).unwrap();
        api.attach_bill_ocr(Arc::new(Recorded(reader)));
        let app = AppCtx {
            api: Some(ApiWorker::spawn(api).unwrap()),
            ..AppCtx::default()
        };
        let setup = OcrSetup {
            dir: dir.to_path_buf(),
            reader: Arc::new(|_dir: &Path| -> Arc<dyn BillOcr> {
                Arc::new(Recorded(ProviderStatus::Ready))
            }),
            fetch,
        };
        Harness {
            ctx: egui::Context::default(),
            screen: AiSettingsScreen::with_ocr_setup(setup),
            app,
            shapes: Vec::new(),
        }
    }

    fn frame(&mut self, events: Vec<Event>) -> String {
        let out = self.ctx.run_ui(
            RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 1400.0))),
                ..RawInput::default()
            },
            |ui| {
                let _stay = self.screen.ui(ui, &mut self.app);
            },
        );
        self.shapes.clone_from(&out.shapes);
        out.drop_without_applying_deltas();
        texts(&self.shapes)
            .into_iter()
            .map(|(t, _)| t)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn settle_on(&mut self, wanted: &str) -> String {
        let mut text = self.frame(Vec::new());
        for _ in 0..2000 {
            if self.screen.loaded() && text.contains(wanted) {
                return text;
            }
            std::thread::sleep(Duration::from_millis(2));
            text = self.frame(Vec::new());
        }
        panic!("never drew {wanted:?}; last frame:\n{text}");
    }

    /// Clicks the drawn text `label` (a button's label) with the pointer.
    fn click(&mut self, label: &str) {
        let at = texts(&self.shapes)
            .into_iter()
            .find(|(t, _)| t == label)
            .unwrap_or_else(|| panic!("{label:?} is not drawn"))
            .1
            .center();
        self.frame(vec![Event::PointerMoved(at)]);
        let press = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        self.frame(vec![press(true)]);
        self.frame(vec![press(false)]);
    }
}

fn kept(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_reader_not_installed_offers_the_download_and_nothing_is_fetched_until_pressed() {
    let dir = folder("s21-ocr-dl-idle");
    let fetch = Arc::new(Recording::default());
    let mut h = Harness::new("s21-ocr-dl-idle.ledgerly", missing(), &dir, fetch.clone());
    let text = h.settle_on(DOWNLOAD);
    assert!(text.contains(MISSING), "{text}");
    assert!(text.contains(SOURCE), "{text}");
    assert!(text.contains(NETWORK), "{text}");
    for _ in 0..50 {
        let _ = h.frame(Vec::new());
    }
    assert_eq!(fetch.urls(), Vec::<String>::new());
    assert_eq!(kept(&dir), Vec::<String>::new());
}

#[test]
fn a_ready_reader_says_ocrs_reads_on_this_computer_with_no_download_button() {
    let dir = folder("s21-ocr-dl-ready");
    let fetch = Arc::new(Recording::default());
    let mut h = Harness::new(
        "s21-ocr-dl-ready.ledgerly",
        ProviderStatus::Ready,
        &dir,
        fetch.clone(),
    );
    let text = h.settle_on("ocrs 0.14.0");
    assert!(
        text.contains("ocrs 0.14.0 on this computer. Both model files are here and load. Every value it reads needs review."),
        "{text}"
    );
    assert!(!text.contains(DOWNLOAD), "{text}");
    assert_eq!(fetch.urls(), Vec::<String>::new());
}

#[test]
fn pressing_download_fetches_the_fixed_url_and_a_mismatch_keeps_nothing() {
    let dir = folder("s21-ocr-dl-mismatch");
    let fetch = Arc::new(Recording::default());
    let (open, gate): (Sender<()>, Receiver<()>) = channel();
    *fetch.gate.lock().unwrap() = Some(gate);
    let mut h = Harness::new(
        "s21-ocr-dl-mismatch.ledgerly",
        missing(),
        &dir,
        fetch.clone(),
    );
    let _ = h.settle_on(DOWNLOAD);
    h.click(DOWNLOAD);
    // While the fetch is held, the screen says it is downloading and the screen keeps drawing.
    let busy = h.settle_on("Downloading the two OCR model files.");
    assert!(
        busy.contains(
            "Downloading the two OCR model files. Bills cannot be read until both are here."
        ),
        "{busy}"
    );
    assert!(
        busy.contains("Only the model download is connected, to ocrs-models.s3-accelerate.amazonaws.com. Bills and books are never sent."),
        "{busy}"
    );
    open.send(()).unwrap();
    let failed = h.settle_on("Download again");
    assert!(
        failed.contains(&format!(
            "The download stopped: the OCR model file text-detection.onnx has checksum \
             2775dbaf0227ae443ccbdcd0e6405322facd655a927344fd7120d50da90c462a, not {}; it was not kept.",
            OCR_MODEL_FILES[0].sha256.unwrap()
        )),
        "{failed}"
    );
    assert!(failed.contains(MISSING), "{failed}");
    // The first file was refused, so the second was never asked for.
    assert_eq!(fetch.urls(), vec![OCR_MODEL_FILES[0].url.to_owned()]);
    assert_eq!(kept(&dir), Vec::<String>::new());
}
