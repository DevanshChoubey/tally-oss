//! The ocrs model files (owner decision 2026-11-27: OCR is `ocrs`, pure Rust).
//!
//! `ocrs` needs two model files, a text detection model and a text recognition model. They are
//! not shipped and never downloaded on their own: the user asks for the download, the files come
//! from the fixed URLs in [`OCR_MODEL_FILES`] (the ones the `ocrs` 0.14.0 crate's
//! `examples/download-models.sh` names), and each file is kept only when its SHA-256 matches the
//! checksum recorded here. The checksums are those of the files the coordinator fetched once with
//! the owner's permission on 2026-11-27 (2,499,479 and 9,713,177 bytes). A file with no recorded
//! checksum is refused before any request.
//!
//! [`ocr_models_status`] says whether the files in a folder load. Missing or unloadable files are
//! [`ProviderStatus::NotInstalled`], with the reason in words the screen shows.
//!
//! [`OcrModels::read_png`] reads a PNG bill with the loaded models into an [`OcrOutput`], one
//! [`OcrLine`] per line `ocrs` recognised, boxed by its characters. `ocrs` gives no confidence, so
//! every line carries [`OCRS_CONFIDENCE_PERMILLE`], the lowest ("needs review", owner decision
//! 2026-11-27): the validator keeps none of these values until the person confirms each one.

use std::fmt;
use std::io::Write as _;
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};

use ledgerly_core::CancelToken;
use ocrs::TextItem as _;

use crate::AssistError;
use crate::ocr::{OcrLine, OcrOutput};
use crate::provider::ProviderStatus;

/// One model file: its name in the models folder, where it is downloaded from, and the SHA-256
/// (lower-case hex) it must have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OcrModelFile {
    /// The file name in the models folder.
    pub name: &'static str,
    /// The fixed https URL it is downloaded from.
    pub url: &'static str,
    /// Its SHA-256, lower-case hex. `None` while no checksum is recorded: the download refuses.
    pub sha256: Option<&'static str>,
}

/// The two files `ocrs` needs, detection first.
pub const OCR_MODEL_FILES: [OcrModelFile; 2] = [
    OcrModelFile {
        name: "text-detection.onnx",
        url: "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx",
        sha256: Some("a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe"),
    },
    OcrModelFile {
        name: "text-recognition.onnx",
        url: "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx",
        sha256: Some("86c145c2edb96c8caed5b1ebb8f44d706408922211451c309c625157dd6061c5"),
    },
];

/// Why the model files cannot be loaded or downloaded.
#[derive(Debug, thiserror::Error)]
pub enum OcrModelsError {
    /// The file is not in the models folder.
    #[error("the OCR model file {file} is not there")]
    Missing {
        /// The file name.
        file: String,
    },
    /// The file is there but is not a model `ocrs` can use.
    #[error("the OCR model file {file} does not load: {reason}")]
    Load {
        /// The file name.
        file: String,
        /// What the loader said.
        reason: String,
    },
    /// No checksum is recorded for the file, so it is not downloaded.
    #[error("no checksum is recorded for the OCR model file {file}, so it is not downloaded")]
    NoChecksum {
        /// The file name.
        file: String,
    },
    /// The download failed.
    #[error("the OCR model file {file} could not be downloaded: {reason}")]
    Fetch {
        /// The file name.
        file: String,
        /// Why.
        reason: String,
    },
    /// The downloaded bytes do not have the recorded checksum; nothing was kept.
    #[error("the OCR model file {file} has checksum {got}, not {expected}; it was not kept")]
    Checksum {
        /// The file name.
        file: String,
        /// The recorded checksum.
        expected: String,
        /// The checksum of what arrived.
        got: String,
    },
    /// The models folder could not be written.
    #[error("the OCR model file {file} could not be written: {reason}")]
    Write {
        /// The file name.
        file: String,
        /// Why.
        reason: String,
    },
}

/// The loaded models, ready for `ocrs`.
pub struct OcrModels {
    engine: ocrs::OcrEngine,
}

impl fmt::Debug for OcrModels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OcrModels").finish_non_exhaustive()
    }
}

impl OcrModels {
    /// Loads both model files from `dir`.
    ///
    /// # Errors
    /// [`OcrModelsError::Missing`] or [`OcrModelsError::Load`] for the first file that is
    /// missing or does not load.
    pub fn load(dir: &Path) -> Result<OcrModels, OcrModelsError> {
        let [detection, recognition] = OCR_MODEL_FILES.map(|f| load_one(dir, f.name));
        let params = ocrs::OcrEngineParams {
            detection_model: Some(detection?),
            recognition_model: Some(recognition?),
            ..ocrs::OcrEngineParams::default()
        };
        let engine = ocrs::OcrEngine::new(params).map_err(|e| OcrModelsError::Load {
            file: OCR_MODEL_FILES[0].name.to_owned(),
            reason: e.to_string(),
        })?;
        Ok(OcrModels { engine })
    }

    /// Reads a PNG image into its text lines, each with the box of its characters and
    /// [`OCRS_CONFIDENCE_PERMILLE`]. Lines with no characters are left out. Boxes are clamped to
    /// the image (a character box can reach past its edge by a pixel).
    ///
    /// # Errors
    /// [`AssistError::Unsupported`] when the bytes are not a PNG, [`AssistError::Output`] when
    /// `ocrs` fails, [`AssistError::Cancelled`] when cancelled between steps.
    pub fn read_png(
        &self,
        png: &[u8],
        sha256: &str,
        cancel: &CancelToken,
    ) -> Result<OcrOutput, AssistError> {
        let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
            .map_err(|e| {
                AssistError::Unsupported(ledgerly_core::Unsupported {
                    what: "reading this image".to_owned(),
                    reason: format!("it is not a PNG the reader can open: {e}"),
                })
            })?
            .into_rgb8();
        let (width, height) = image.dimensions();
        let failed = |e: &dyn fmt::Display| AssistError::Output {
            reason: format!("the OCR engine failed: {e}"),
        };
        let source = ocrs::ImageSource::from_bytes(image.as_raw(), (width, height))
            .map_err(|e| failed(&e))?;
        let input = self.engine.prepare_input(source).map_err(|e| failed(&e))?;
        let words = self.engine.detect_words(&input).map_err(|e| failed(&e))?;
        if cancel.is_cancelled() {
            return Err(AssistError::Cancelled);
        }
        let line_rects = self.engine.find_text_lines(&input, &words);
        let recognised = self
            .engine
            .recognize_text(&input, &line_rects)
            .map_err(|e| failed(&e))?;
        if cancel.is_cancelled() {
            return Err(AssistError::Cancelled);
        }
        let lines = recognised
            .iter()
            .flatten()
            .filter_map(|line| ocr_line(line, width, height))
            .collect();
        Ok(OcrOutput {
            doc_hash: sha256.to_owned(),
            lines,
        })
    }
}

/// The confidence every value read by `ocrs` carries: the lowest, "needs review". `ocrs` reports
/// none, and a made-up one would be a silent default.
pub const OCRS_CONFIDENCE_PERMILLE: u16 = crate::review::NEEDS_REVIEW_PERMILLE;

/// One recognised line as an [`OcrLine`] on page 1, or `None` when it has no characters.
fn ocr_line(line: &ocrs::TextLine, width: u32, height: u32) -> Option<OcrLine> {
    let chars = line.chars();
    let clamp = |v: i32, max: u32| u32::try_from(v.max(0)).map_or(max, |v| v.min(max));
    let left = chars.iter().map(|c| c.rect.left()).min()?;
    let top = chars.iter().map(|c| c.rect.top()).min()?;
    let right = chars.iter().map(|c| c.rect.right()).max()?;
    let bottom = chars.iter().map(|c| c.rect.bottom()).max()?;
    let (x, y) = (clamp(left, width), clamp(top, height));
    let (r, b) = (clamp(right, width), clamp(bottom, height));
    Some(OcrLine {
        page: 1,
        text: line.to_string(),
        bbox: [x, y, r.saturating_sub(x), b.saturating_sub(y)],
        confidence_permille: OCRS_CONFIDENCE_PERMILLE,
    })
}

fn load_one(dir: &Path, name: &str) -> Result<rten::Model, OcrModelsError> {
    let path = dir.join(name);
    if !path.is_file() {
        return Err(OcrModelsError::Missing {
            file: name.to_owned(),
        });
    }
    rten::Model::load_file(&path).map_err(|e| OcrModelsError::Load {
        file: name.to_owned(),
        reason: e.to_string(),
    })
}

/// Whether the model files in `dir` load. Missing files are named together, so the screen can
/// say everything that is missing at once.
pub fn ocr_models_status(dir: &Path) -> ProviderStatus {
    let missing: Vec<&str> = OCR_MODEL_FILES
        .iter()
        .filter(|f| !dir.join(f.name).is_file())
        .map(|f| f.name)
        .collect();
    if !missing.is_empty() {
        return ProviderStatus::NotInstalled {
            reason: format!("the OCR model files are not there: {}", missing.join(", ")),
        };
    }
    match OcrModels::load(dir) {
        Ok(_) => ProviderStatus::Ready,
        Err(e) => ProviderStatus::NotInstalled {
            reason: e.to_string(),
        },
    }
}

/// Fetches the bytes at an https URL. Tests use a recording double; the app uses
/// [`UreqModelFetch`] only when the user asks for the download.
pub trait ModelFetch: Send + Sync {
    /// The body at `url`.
    ///
    /// # Errors
    /// [`AssistError::Network`] when no body arrives.
    fn get(&self, url: &str) -> Result<Vec<u8>, AssistError>;
}

/// The largest model file accepted (the two files are about 10 MB together).
const MAX_MODEL_BYTES: u64 = 64 * 1024 * 1024;

/// HTTPS over `ureq`: https only, no redirects, a global timeout, a size limit.
#[derive(Debug)]
pub struct UreqModelFetch {
    agent: ureq::Agent,
}

impl UreqModelFetch {
    /// A fetcher with a ten-minute global timeout.
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(0)
            .timeout_global(Some(Duration::from_secs(600)))
            .build();
        UreqModelFetch {
            agent: ureq::Agent::new_with_config(config),
        }
    }
}

impl Default for UreqModelFetch {
    fn default() -> Self {
        UreqModelFetch::new()
    }
}

impl ModelFetch for UreqModelFetch {
    fn get(&self, url: &str) -> Result<Vec<u8>, AssistError> {
        let network = |e: ureq::Error| AssistError::Network {
            reason: e.to_string(),
        };
        let mut response = self.agent.get(url).call().map_err(network)?;
        response
            .body_mut()
            .with_config()
            .limit(MAX_MODEL_BYTES)
            .read_to_vec()
            .map_err(network)
    }
}

/// Downloads both model files into `dir`, each kept only when its checksum matches. Refuses
/// before any request when a file has no recorded checksum.
///
/// # Errors
/// [`OcrModelsError`] for the first file that cannot be downloaded or kept.
pub fn download_ocr_models(dir: &Path, fetch: &dyn ModelFetch) -> Result<(), OcrModelsError> {
    if let Some(file) = OCR_MODEL_FILES.iter().find(|f| f.sha256.is_none()) {
        return Err(OcrModelsError::NoChecksum {
            file: file.name.to_owned(),
        });
    }
    for file in &OCR_MODEL_FILES {
        download_model(file, dir, fetch)?;
    }
    Ok(())
}

/// Downloads one model file into `dir`. The bytes are checked before anything is written, and
/// written to a side file that is renamed into place, so a failed or mismatched download leaves
/// nothing behind.
///
/// # Errors
/// [`OcrModelsError::NoChecksum`], [`OcrModelsError::Fetch`], [`OcrModelsError::Checksum`] or
/// [`OcrModelsError::Write`].
pub fn download_model(
    file: &OcrModelFile,
    dir: &Path,
    fetch: &dyn ModelFetch,
) -> Result<(), OcrModelsError> {
    let name = file.name.to_owned();
    let Some(expected) = file.sha256 else {
        return Err(OcrModelsError::NoChecksum { file: name });
    };
    let bytes = fetch.get(file.url).map_err(|e| OcrModelsError::Fetch {
        file: name.clone(),
        reason: e.to_string(),
    })?;
    let got = hex(&Sha256::digest(&bytes));
    if got != expected.to_ascii_lowercase() {
        return Err(OcrModelsError::Checksum {
            file: name,
            expected: expected.to_owned(),
            got,
        });
    }
    let part = dir.join(format!("{}.part", file.name));
    let write = |e: std::io::Error| OcrModelsError::Write {
        file: name.clone(),
        reason: e.to_string(),
    };
    let written = std::fs::File::create(&part)
        .and_then(|mut out| out.write_all(&bytes).and_then(|()| out.sync_all()))
        .and_then(|()| std::fs::rename(&part, dir.join(file.name)));
    if let Err(e) = written {
        let _removed = std::fs::remove_file(&part);
        return Err(write(e));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}
