//! Round s21: the ocrs model files (owner decision 2026-11-27: OCR is ocrs; its model files are
//! downloaded only when the user asks, from a fixed URL, with a checksum check).
//!
//! Provenance (derived). The file names and URLs are the ones in the ocrs 0.14.0 crate's
//! `examples/download-models.sh`. No real model file is used or downloaded here: the "model"
//! files are a few bytes written by the test, and the fetcher is a recording double, so nothing
//! opens a connection. The checksums are SHA-256 of the test's own bytes, computed in the test.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::sync::Mutex;

use ledgerly_assist::AssistError;
use ledgerly_assist::ocr_models::{
    ModelFetch, OCR_MODEL_FILES, OcrModelFile, OcrModels, OcrModelsError, download_model,
    download_ocr_models, ocr_models_status,
};
use ledgerly_assist::provider::ProviderStatus;
use sha2::{Digest, Sha256};

fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

#[derive(Default)]
struct Recorded {
    answer: Option<Vec<u8>>,
    asked: Mutex<Vec<String>>,
}

impl ModelFetch for Recorded {
    fn get(&self, url: &str) -> Result<Vec<u8>, AssistError> {
        self.asked.lock().unwrap().push(url.to_owned());
        self.answer.clone().ok_or_else(|| AssistError::Network {
            reason: "the recording has no answer".to_owned(),
        })
    }
}

fn test_file(sha256: &'static str) -> OcrModelFile {
    OcrModelFile {
        name: "text-detection.onnx",
        url: "https://example.invalid/text-detection.onnx",
        sha256: Some(sha256),
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

#[test]
fn the_fixed_files_are_the_two_ocrs_models_over_https() {
    let names: Vec<&str> = OCR_MODEL_FILES.iter().map(|f| f.name).collect();
    assert_eq!(names, ["text-detection.onnx", "text-recognition.onnx"]);
    assert_eq!(
        OCR_MODEL_FILES[0].url,
        "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx"
    );
    assert_eq!(
        OCR_MODEL_FILES[1].url,
        "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx"
    );
}

#[test]
fn with_no_model_files_ocr_is_not_installed_and_names_both_files() {
    let dir = dir("s21-ocr-empty");
    match ocr_models_status(&dir) {
        ProviderStatus::NotInstalled { reason } => {
            assert!(reason.contains("text-detection.onnx"), "{reason}");
            assert!(reason.contains("text-recognition.onnx"), "{reason}");
        }
        other @ (ProviderStatus::Ready | ProviderStatus::NotConfigured { .. }) => {
            panic!("expected Not installed, got {other:?}")
        }
    }
    match OcrModels::load(&dir) {
        Err(OcrModelsError::Missing { file }) => assert_eq!(file, "text-detection.onnx"),
        other => panic!("expected a missing file, got {other:?}"),
    }
}

#[test]
fn a_file_that_is_not_a_model_is_a_typed_load_error_not_a_panic() {
    let dir = dir("s21-ocr-garbage");
    for f in &OCR_MODEL_FILES {
        std::fs::write(dir.join(f.name), b"not a model").unwrap();
    }
    match OcrModels::load(&dir) {
        Err(OcrModelsError::Load { file, reason }) => {
            assert_eq!(file, "text-detection.onnx");
            assert_ne!(reason.len(), 0);
        }
        other => panic!("expected a load error, got {other:?}"),
    }
    match ocr_models_status(&dir) {
        ProviderStatus::NotInstalled { reason } => {
            assert!(reason.contains("does not load"), "{reason}");
        }
        other @ (ProviderStatus::Ready | ProviderStatus::NotConfigured { .. }) => {
            panic!("expected Not installed, got {other:?}")
        }
    }
}

#[test]
fn the_fixed_checksums_are_the_ones_of_the_files_the_owner_fetched() {
    // provenance: observed. The coordinator fetched both files from the fixed URLs on
    // 2026-11-27 with the owner's permission and reported their SHA-256 (2,499,479 and
    // 9,713,177 bytes); this session's sha256sum of the same files agreed.
    assert_eq!(
        OCR_MODEL_FILES[0].sha256,
        Some("a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe")
    );
    assert_eq!(
        OCR_MODEL_FILES[1].sha256,
        Some("86c145c2edb96c8caed5b1ebb8f44d706408922211451c309c625157dd6061c5")
    );
}

#[test]
fn the_download_of_the_fixed_files_refuses_bytes_whose_hash_differs_and_keeps_nothing() {
    let dir = dir("s21-ocr-fixed-mismatch");
    let fetch = Recorded {
        answer: Some(b"not the model".to_vec()),
        ..Recorded::default()
    };
    match download_ocr_models(&dir, &fetch) {
        Err(OcrModelsError::Checksum { file, expected, .. }) => {
            assert_eq!(file, "text-detection.onnx");
            assert_eq!(Some(expected.as_str()), OCR_MODEL_FILES[0].sha256);
        }
        other => panic!("expected a checksum refusal, got {other:?}"),
    }
    // It stops at the first file: the second is never asked for.
    assert_eq!(
        *fetch.asked.lock().unwrap(),
        ["https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx"]
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
}

#[test]
fn a_file_with_no_recorded_checksum_is_refused_before_any_request() {
    let dir = dir("s21-ocr-nochecksum");
    let fetch = Recorded {
        answer: Some(b"anything".to_vec()),
        ..Recorded::default()
    };
    let file = OcrModelFile {
        sha256: None,
        ..OCR_MODEL_FILES[0]
    };
    match download_model(&file, &dir, &fetch) {
        Err(OcrModelsError::NoChecksum { file }) => assert_eq!(file, "text-detection.onnx"),
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(fetch.asked.lock().unwrap().len(), 0);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
}

#[test]
fn a_file_whose_checksum_matches_is_kept() {
    let dir = dir("s21-ocr-match");
    let bytes = b"model bytes".to_vec();
    let sha = leak(hex(&Sha256::digest(&bytes)));
    let fetch = Recorded {
        answer: Some(bytes.clone()),
        ..Recorded::default()
    };
    download_model(&test_file(sha), &dir, &fetch).unwrap();
    assert_eq!(
        *fetch.asked.lock().unwrap(),
        ["https://example.invalid/text-detection.onnx"]
    );
    assert_eq!(
        std::fs::read(dir.join("text-detection.onnx")).unwrap(),
        bytes
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
}

#[test]
fn a_file_whose_checksum_differs_is_refused_and_nothing_is_left() {
    let dir = dir("s21-ocr-mismatch");
    let expected = leak(hex(&Sha256::digest(b"the real model")));
    let fetch = Recorded {
        answer: Some(b"something else".to_vec()),
        ..Recorded::default()
    };
    match download_model(&test_file(expected), &dir, &fetch) {
        Err(OcrModelsError::Checksum {
            file,
            expected: e,
            got,
        }) => {
            assert_eq!(file, "text-detection.onnx");
            assert_eq!(e, expected);
            assert_eq!(got, hex(&Sha256::digest(b"something else")));
        }
        other => panic!("expected a checksum refusal, got {other:?}"),
    }
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
}

#[test]
fn a_failed_request_is_a_fetch_error_and_nothing_is_written() {
    let dir = dir("s21-ocr-fetchfail");
    let fetch = Recorded::default();
    match download_model(&test_file("00"), &dir, &fetch) {
        Err(OcrModelsError::Fetch { file, .. }) => assert_eq!(file, "text-detection.onnx"),
        other => panic!("expected a fetch error, got {other:?}"),
    }
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
}
