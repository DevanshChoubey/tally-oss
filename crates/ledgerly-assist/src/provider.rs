//! Providers: what answers the assistant's requests (round s21, spec 6.8).
//!
//! A provider is the model behind the assistant: the local model on this computer, the user's
//! cloud provider (bring your own key), or the fixed-answer test provider. The app never calls a
//! provider directly; it calls the one assistant seam, [`Proposer`], through [`ProviderProposer`],
//! which turns each input into a typed [`Proposal`] with the same readers for every provider:
//!
//! * plain words go to the provider's text model and are read by [`crate::nl_entry::NlEntry`]
//!   (amounts are parsed by the reader, never by the model);
//! * a bill image goes to the provider's OCR and is read by [`crate::ocr::read_bill`];
//! * a question goes to the provider and comes back as a [`ReportQuery`], which the report engine
//!   runs.
//!
//! Three providers exist:
//!
//! * [`LocalModel`] points at a model file the user chose. No inference runtime is linked into
//!   this build (choosing one is the owner's decision, as is any download), so it reports
//!   [`ProviderStatus::NotInstalled`] and every request is `Unsupported`. It never touches the
//!   network.
//! * [`CloudModel`] wraps [`crate::cloud::CloudClient`] (redaction first, key from the OS
//!   keychain). It refuses documents: pixels cannot be redacted.
//! * [`FixedAnswers`] answers from recordings given to it, for tests and screen captures. It
//!   feeds the real readers, so every figure still comes from the readers and the engine. An input
//!   it has no recording for is an error, never a guess.

use std::fmt;
use std::path::PathBuf;

use ledgerly_core::{CancelToken, Unsupported};
use ledgerly_model::Masters;

use crate::cloud::CloudClient;
use crate::nl_entry::{MasterLedger, ModelClient, ModelRequest, NlEntry};
use crate::ocr::{OcrOutput, read_bill};
use crate::{AssistError, Proposal, ProposalInput, Proposer, ReportQuery};

/// Where a provider runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderPlace {
    /// On this computer. Nothing leaves it.
    ThisComputer,
    /// At the user's cloud provider, with their key.
    Cloud,
    /// The fixed-answer test provider (tests and captures only).
    Test,
}

/// What a provider says about itself, for the AI activity log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderInfo {
    /// Its name, as the log shows it.
    pub name: String,
    /// Its version, as it states it.
    pub version: String,
    /// Where it runs.
    pub place: ProviderPlace,
}

/// Whether a provider can answer now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderStatus {
    /// It can answer.
    Ready,
    /// It cannot answer: no model or runtime is installed. The text says what is missing.
    NotInstalled {
        /// What is missing, in words the screen shows.
        reason: String,
    },
    /// It cannot answer: the cloud key or configuration is missing.
    NotConfigured {
        /// What is missing.
        reason: String,
    },
}

/// A model behind the assistant. Implementations: [`LocalModel`], [`CloudModel`],
/// [`FixedAnswers`].
pub trait AiProvider: Send + fmt::Debug {
    /// The provider's name, version and place.
    fn info(&self) -> ProviderInfo;

    /// Whether it can answer now.
    fn status(&self) -> ProviderStatus;

    /// Plain words to a draft voucher, read with the ledgers names resolve against.
    ///
    /// # Errors
    /// [`AssistError`] when no draft can be produced.
    fn read_text(
        &self,
        text: &str,
        ledgers: &[MasterLedger],
        cancel: &CancelToken,
    ) -> Result<Proposal, AssistError>;

    /// The text lines of a document, with their boxes and confidences.
    ///
    /// # Errors
    /// [`AssistError`] when the document cannot be read.
    fn ocr(
        &self,
        bytes: &[u8],
        sha256: &str,
        cancel: &CancelToken,
    ) -> Result<OcrOutput, AssistError>;

    /// A question about the books to a report query.
    ///
    /// # Errors
    /// [`AssistError`] when no query can be produced.
    fn ask(&self, question: &str, cancel: &CancelToken) -> Result<ReportQuery, AssistError>;
}

/// The assistant seam over a provider and the masters of the open company.
#[derive(Debug)]
pub struct ProviderProposer<'a> {
    provider: &'a dyn AiProvider,
    masters: &'a Masters,
}

impl<'a> ProviderProposer<'a> {
    /// The seam over `provider`, resolving names against `masters`.
    pub fn new(provider: &'a dyn AiProvider, masters: &'a Masters) -> Self {
        ProviderProposer { provider, masters }
    }
}

impl Proposer for ProviderProposer<'_> {
    fn propose(
        &self,
        input: &ProposalInput,
        cancel: &CancelToken,
    ) -> Result<Proposal, AssistError> {
        if cancel.is_cancelled() {
            return Err(AssistError::Cancelled);
        }
        match input {
            ProposalInput::Text(text) => {
                let ledgers: Vec<MasterLedger> = self
                    .masters
                    .ledgers
                    .values()
                    .map(MasterLedger::from)
                    .collect();
                self.provider.read_text(text, &ledgers, cancel)
            }
            ProposalInput::Document { bytes, sha256, .. } => {
                let ocr = self.provider.ocr(bytes, sha256, cancel)?;
                if ocr.doc_hash != *sha256 {
                    return Err(AssistError::Output {
                        reason: "the reader answered for a different document".to_owned(),
                    });
                }
                let reading = read_bill(&ocr, self.masters)?;
                Ok(Proposal::VoucherDraft(Box::new(reading.draft)))
            }
            ProposalInput::Question(question) => self
                .provider
                .ask(question, cancel)
                .map(Proposal::ReportQuery),
            ProposalInput::BankLine { .. } | ProposalInput::Columns { .. } => {
                Err(AssistError::Unsupported(Unsupported {
                    what: "this assistant request".to_owned(),
                    reason:
                        "bank lines and import columns are not sent to a provider in this build"
                            .to_owned(),
                }))
            }
        }
    }
}

/// The local model: a model file the user points to. No inference runtime is linked into this
/// build, so it is never [`ProviderStatus::Ready`]; it never touches the network.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LocalModel {
    path: Option<PathBuf>,
}

/// What the local model file looks like on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalModelFile {
    /// No file was chosen.
    NotChosen,
    /// A file was chosen and is not there.
    Missing(PathBuf),
    /// A file was chosen and is there, with its size in bytes.
    Found {
        /// The file.
        path: PathBuf,
        /// Its size on disk.
        bytes: u64,
    },
}

/// The reason the local model cannot answer in this build.
const NO_RUNTIME: &str = "no local inference runtime is built into this version of Ledgerly";

impl LocalModel {
    /// The local model at `path` (the file the user chose), or none.
    pub fn new(path: Option<PathBuf>) -> Self {
        LocalModel { path }
    }

    /// The chosen file and whether it is there. Reads the file's size only.
    pub fn file(&self) -> LocalModelFile {
        match &self.path {
            None => LocalModelFile::NotChosen,
            Some(path) => match std::fs::metadata(path) {
                Ok(meta) if meta.is_file() => LocalModelFile::Found {
                    path: path.clone(),
                    bytes: meta.len(),
                },
                Ok(_) | Err(_) => LocalModelFile::Missing(path.clone()),
            },
        }
    }

    fn refuse<T>(&self) -> Result<T, AssistError> {
        Err(AssistError::Unsupported(Unsupported {
            what: "the local model".to_owned(),
            reason: not_installed_reason(&self.file()),
        }))
    }
}

/// Why the local model cannot answer, in words the screen shows.
fn not_installed_reason(file: &LocalModelFile) -> String {
    match file {
        LocalModelFile::NotChosen => format!("no model file is chosen, and {NO_RUNTIME}"),
        LocalModelFile::Missing(path) => format!(
            "the model file {} is not there, and {NO_RUNTIME}",
            path.display()
        ),
        LocalModelFile::Found { .. } => format!("the model file is there, but {NO_RUNTIME}"),
    }
}

impl AiProvider for LocalModel {
    fn info(&self) -> ProviderInfo {
        let name = match &self.path {
            Some(path) => path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            ),
            None => "local model".to_owned(),
        };
        ProviderInfo {
            name,
            version: String::new(),
            place: ProviderPlace::ThisComputer,
        }
    }

    fn status(&self) -> ProviderStatus {
        ProviderStatus::NotInstalled {
            reason: not_installed_reason(&self.file()),
        }
    }

    fn read_text(
        &self,
        _text: &str,
        _ledgers: &[MasterLedger],
        _cancel: &CancelToken,
    ) -> Result<Proposal, AssistError> {
        self.refuse()
    }

    fn ocr(
        &self,
        _bytes: &[u8],
        _sha256: &str,
        _cancel: &CancelToken,
    ) -> Result<OcrOutput, AssistError> {
        self.refuse()
    }

    fn ask(&self, _question: &str, _cancel: &CancelToken) -> Result<ReportQuery, AssistError> {
        self.refuse()
    }
}

/// The user's cloud provider, with their key (privacy tier 2). Text only: documents are refused
/// because an image cannot be redacted.
#[derive(Debug)]
pub struct CloudModel {
    client: CloudClient,
}

impl CloudModel {
    /// The provider over `client`.
    pub fn new(client: CloudClient) -> Self {
        CloudModel { client }
    }
}

impl AiProvider for CloudModel {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: self.client.config().model.clone(),
            version: crate::cloud::PROMPT_TEMPLATE_VERSION.to_owned(),
            place: ProviderPlace::Cloud,
        }
    }

    fn status(&self) -> ProviderStatus {
        ProviderStatus::Ready
    }

    fn read_text(
        &self,
        text: &str,
        _ledgers: &[MasterLedger],
        cancel: &CancelToken,
    ) -> Result<Proposal, AssistError> {
        self.client
            .propose(&ProposalInput::Text(text.to_owned()), cancel)
    }

    fn ocr(
        &self,
        _bytes: &[u8],
        _sha256: &str,
        _cancel: &CancelToken,
    ) -> Result<OcrOutput, AssistError> {
        Err(AssistError::Unsupported(Unsupported {
            what: "reading a document in the cloud".to_owned(),
            reason: "an image cannot be redacted before it leaves this computer".to_owned(),
        }))
    }

    fn ask(&self, question: &str, cancel: &CancelToken) -> Result<ReportQuery, AssistError> {
        match self
            .client
            .propose(&ProposalInput::Question(question.to_owned()), cancel)?
        {
            Proposal::ReportQuery(q) => Ok(q),
            Proposal::VoucherDraft(_)
            | Proposal::MatchSuggestion(_)
            | Proposal::MappingSuggestion(_) => Err(AssistError::Output {
                reason: "the provider answered a question with something other than a report query"
                    .to_owned(),
            }),
        }
    }
}

/// Answers from recordings: the test provider for tests and screen captures. Its text answers go
/// through the real plain-words reader and its OCR lines through the real bill reader, so it
/// supplies transcriptions only, never a figure. Anything without a recording is an error.
#[derive(Debug, Default, Clone)]
pub struct FixedAnswers {
    texts: Vec<(String, String)>,
    documents: Vec<OcrOutput>,
    questions: Vec<(String, ReportQuery)>,
    not_installed: Option<String>,
}

/// The name the test provider gives itself in the AI activity log.
pub const FIXED_ANSWERS_NAME: &str = "Fixed answers (test provider)";

impl FixedAnswers {
    /// A provider with no recordings.
    pub fn new() -> Self {
        FixedAnswers::default()
    }

    /// Records the model's JSON answer (the plain-words reader's shape) for the typed `text`.
    #[must_use]
    pub fn with_text(mut self, text: impl Into<String>, model_json: impl Into<String>) -> Self {
        self.texts.push((text.into(), model_json.into()));
        self
    }

    /// Records the OCR lines of a document (keyed by its hash).
    #[must_use]
    pub fn with_document(mut self, ocr: OcrOutput) -> Self {
        self.documents.push(ocr);
        self
    }

    /// Records the report query for a question.
    #[must_use]
    pub fn with_question(mut self, question: impl Into<String>, query: ReportQuery) -> Self {
        self.questions.push((question.into(), query));
        self
    }

    /// Makes the provider report itself not installed, as a local model with no runtime does.
    #[must_use]
    pub fn not_installed(mut self, reason: impl Into<String>) -> Self {
        self.not_installed = Some(reason.into());
        self
    }

    fn check(&self, cancel: &CancelToken) -> Result<(), AssistError> {
        if cancel.is_cancelled() {
            return Err(AssistError::Cancelled);
        }
        match &self.not_installed {
            Some(reason) => Err(AssistError::Unsupported(Unsupported {
                what: "the local model".to_owned(),
                reason: reason.clone(),
            })),
            None => Ok(()),
        }
    }
}

/// The recorded text answers as a model client for the plain-words reader.
#[derive(Debug)]
struct Recorded<'a>(&'a [(String, String)]);

impl ModelClient for Recorded<'_> {
    fn complete(
        &self,
        request: &ModelRequest,
        _cancel: &CancelToken,
    ) -> Result<String, AssistError> {
        self.0
            .iter()
            .find(|(text, _)| *text == request.user_text)
            .map(|(_, json)| json.clone())
            .ok_or_else(|| AssistError::Output {
                reason: "the test provider has no answer recorded for these words".to_owned(),
            })
    }
}

impl AiProvider for FixedAnswers {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: FIXED_ANSWERS_NAME.to_owned(),
            version: "s21-1".to_owned(),
            place: ProviderPlace::Test,
        }
    }

    fn status(&self) -> ProviderStatus {
        match &self.not_installed {
            Some(reason) => ProviderStatus::NotInstalled {
                reason: reason.clone(),
            },
            None => ProviderStatus::Ready,
        }
    }

    fn read_text(
        &self,
        text: &str,
        ledgers: &[MasterLedger],
        cancel: &CancelToken,
    ) -> Result<Proposal, AssistError> {
        self.check(cancel)?;
        NlEntry::new(Recorded(&self.texts), ledgers.to_vec())
            .propose(&ProposalInput::Text(text.to_owned()), cancel)
    }

    fn ocr(
        &self,
        _bytes: &[u8],
        sha256: &str,
        cancel: &CancelToken,
    ) -> Result<OcrOutput, AssistError> {
        self.check(cancel)?;
        self.documents
            .iter()
            .find(|d| d.doc_hash == sha256)
            .cloned()
            .ok_or_else(|| AssistError::Output {
                reason: "the test provider has no reading recorded for this document".to_owned(),
            })
    }

    fn ask(&self, question: &str, cancel: &CancelToken) -> Result<ReportQuery, AssistError> {
        self.check(cancel)?;
        self.questions
            .iter()
            .find(|(q, _)| q == question)
            .map(|(_, query)| query.clone())
            .ok_or_else(|| AssistError::Output {
                reason: "the test provider has no query recorded for this question".to_owned(),
            })
    }
}
