//! The local bill reader's setup (round s21): where its model files live, the reader made over
//! that folder, and the fetcher the download uses.
//!
//! The real window registers [`OcrSetup::system`] (the `ocrs` reader and `UreqModelFetch`) with
//! [`use_ocr_setup`], the way it registers the OS keychain; the capture tool registers a recorded
//! reader and a recording fetcher, and tests hand theirs to the screen
//! (`AiSettingsScreen::with_ocr_setup`). With no setup the AI and privacy screen offers no
//! download and no reader is attached. Opening a company
//! only attaches a reader over the folder, which loads nothing and opens no connection; the
//! fetcher is used only when the person presses Download.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ledgerly_api::{BillOcr, ModelFetch, OcrsReader, UreqModelFetch};

/// Makes the reader over a models folder.
pub type ReaderMaker = Arc<dyn Fn(&Path) -> Arc<dyn BillOcr> + Send + Sync>;

/// The models folder, the reader maker and the fetcher.
#[derive(Clone)]
pub struct OcrSetup {
    /// The folder the two model files are kept in.
    pub dir: PathBuf,
    /// Makes the reader over [`OcrSetup::dir`]; called when a company opens and after a download.
    pub reader: ReaderMaker,
    /// Fetches a model file; called only when the person presses Download.
    pub fetch: Arc<dyn ModelFetch>,
}

impl fmt::Debug for OcrSetup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OcrSetup")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

impl OcrSetup {
    /// The real window's setup: the `ocrs` reader and the https fetcher, over `dir`.
    pub fn system(dir: PathBuf) -> Self {
        OcrSetup {
            dir,
            reader: Arc::new(|dir: &Path| -> Arc<dyn BillOcr> { Arc::new(OcrsReader::new(dir)) }),
            fetch: Arc::new(UreqModelFetch::new()),
        }
    }

    /// The reader over the folder.
    pub fn make_reader(&self) -> Arc<dyn BillOcr> {
        (self.reader)(&self.dir)
    }
}

/// The setup every company opened from now on gets, while the release carries AI.
static OCR_SETUP: Mutex<Option<OcrSetup>> = Mutex::new(None);

/// Registers the OCR setup. The real window and the capture tool call this; tests do not.
pub fn use_ocr_setup(setup: OcrSetup) {
    if let Ok(mut slot) = OCR_SETUP.lock() {
        *slot = Some(setup);
    }
}

/// The registered setup, if any.
pub fn ocr_setup() -> Option<OcrSetup> {
    OCR_SETUP.lock().ok().and_then(|slot| slot.clone())
}
