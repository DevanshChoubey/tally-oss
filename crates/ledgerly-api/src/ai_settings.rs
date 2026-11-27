//! AI settings beyond the tier (round s21, plan round B; spec 6.8, 9): the local model file, the
//! cloud provider the user describes, and the cloud key.
//!
//! The model file and the provider are audited settings in the company file (`SettingChanged`
//! under [`LOCAL_MODEL_FILE_KEY`] and `ai.cloud.*`). The key is never in the company file: it is
//! kept in the attached [`KeyVault`] (the OS keychain in the app) under [`KEY_SERVICE`] and an
//! account named after the company. Reading these settings touches the vault only when one is
//! attached, and never the network.

use std::path::{Path, PathBuf};

use std::sync::Arc;

use ledgerly_assist::cloud::{
    CloudClient, CloudConfig, CloudProvider, CloudRequest, CloudResponse, CloudTransport, KeyStore,
    KeyVault,
};
use ledgerly_assist::nl_entry::MasterLedger;
use ledgerly_assist::provider::{AiProvider, LocalModel, LocalModelFile, ProviderStatus};
use ledgerly_assist::provider::{CloudModel, ProviderInfo, ProviderPlace};
use ledgerly_assist::{AssistError, Proposal};
use ledgerly_core::CancelToken;
use ledgerly_core::{Outcome, Unsupported};
use ledgerly_model::AiFeature;
use ledgerly_store::Event;

use crate::error::ApiError;
use crate::events::read_all;
use crate::facade::Api;

/// The setting key of the local model file (its path, or empty for none).
pub const LOCAL_MODEL_FILE_KEY: &str = "ai.local.model_file";
/// The setting key of the cloud endpoint URL.
const CLOUD_ENDPOINT_KEY: &str = "ai.cloud.endpoint";
/// The setting key of the cloud request format.
const CLOUD_FORMAT_KEY: &str = "ai.cloud.format";
/// The setting key of the cloud model name for the features without their own model.
const CLOUD_MODEL_KEY: &str = "ai.cloud.model";
/// The setting key of the cloud model name for bill reading.
const CLOUD_BILL_MODEL_KEY: &str = "ai.cloud.bill_model";
/// The setting key of the cloud model name for plain words.
const CLOUD_WORDS_MODEL_KEY: &str = "ai.cloud.words_model";
/// The Anthropic Messages API's request URL, used when the Messages format names no endpoint.
pub const ANTHROPIC_MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
/// The model bill reading uses with the Messages format when the user names none (owner's
/// decision, 2026-11-27).
pub const DEFAULT_BILL_MODEL: &str = "claude-sonnet-5-5";
/// The model plain words use with the Messages format when the user names none (owner's
/// decision, 2026-11-27).
pub const DEFAULT_WORDS_MODEL: &str = "claude-haiku-5-5";
/// The output token limit sent with each request (the Messages API requires one). A proposal is
/// one short JSON object; 1024 tokens is ample for it.
const MAX_OUTPUT_TOKENS: u32 = 1024;
/// The keychain service the cloud key is saved under.
pub const KEY_SERVICE: &str = "ledgerly-ai-cloud";

/// The cloud provider as the user described it. Every part is the user's choice (bring your own
/// key); none has a default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CloudSettings {
    /// The endpoint URL.
    pub endpoint: Option<String>,
    /// The request format: `messages` (the Anthropic Messages API, where the endpoint may be
    /// left out) or `chat-completions` (an OpenAI-compatible chat endpoint, where the endpoint is
    /// the base URL and `/chat/completions` is added to it).
    pub format: Option<String>,
    /// The model name for the features without their own model below.
    pub model: Option<String>,
    /// The model name for bill reading.
    pub bill_model: Option<String>,
    /// The model name for plain words.
    pub words_model: Option<String>,
}

/// The model `feature` is sent to: its own model when named; with the Messages format, the
/// owner's default for bill reading and plain words; else the model for other features.
#[must_use]
pub fn cloud_model_for(cloud: &CloudSettings, feature: AiFeature) -> Option<String> {
    let messages = cloud.format.as_deref() == Some(CLOUD_FORMATS[1]);
    let (own, default) = match feature {
        AiFeature::BillReading => (&cloud.bill_model, DEFAULT_BILL_MODEL),
        AiFeature::PlainWords => (&cloud.words_model, DEFAULT_WORDS_MODEL),
        AiFeature::BankSuggestion | AiFeature::Ask | AiFeature::Gstr2bExplanation => {
            return cloud.model.clone();
        }
    };
    own.clone()
        .or_else(|| messages.then(|| default.to_owned()))
        .or_else(|| cloud.model.clone())
}

/// The URL requests go to: the endpoint (or [`ANTHROPIC_MESSAGES_URL`]) for the Messages format,
/// the base URL with `/chat/completions` for the chat format; `None` when there is none.
#[must_use]
pub fn cloud_request_url(cloud: &CloudSettings) -> Option<String> {
    match cloud.format.as_deref() {
        Some(f) if f == CLOUD_FORMATS[1] => Some(
            cloud
                .endpoint
                .clone()
                .unwrap_or_else(|| ANTHROPIC_MESSAGES_URL.to_owned()),
        ),
        Some(f) if f == CLOUD_FORMATS[0] => cloud
            .endpoint
            .as_deref()
            .map(|base| format!("{}/chat/completions", base.trim_end_matches('/'))),
        Some(_) | None => None,
    }
}

/// The request formats the cloud client speaks, as written in [`CloudSettings::format`].
pub const CLOUD_FORMATS: [&str; 2] = ["chat-completions", "messages"];

/// Why `cloud` cannot be stored: an endpoint the client would never send to, or a request format
/// it does not speak. A part left out is not refused (the cloud choice names what is missing).
fn cloud_settings_refusal(cloud: &CloudSettings) -> Option<Unsupported> {
    let what = "the cloud provider".to_owned();
    if let Some(endpoint) = cloud.endpoint.as_deref()
        && !ledgerly_assist::cloud::is_https(endpoint)
    {
        return Some(Unsupported {
            what,
            reason: "the endpoint must start with https://, so the key is never sent in clear"
                .to_owned(),
        });
    }
    if let Some(format) = cloud.format.as_deref()
        && !CLOUD_FORMATS.contains(&format)
    {
        return Some(Unsupported {
            what,
            reason: format!(
                "the request format must be {} or {}",
                CLOUD_FORMATS[0], CLOUD_FORMATS[1]
            ),
        });
    }
    None
}

/// Whether the cloud key is saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyState {
    /// No key vault is attached, so no key can be saved or read.
    NoVault,
    /// The vault holds no key for this company.
    NotSet,
    /// The vault holds a key for this company.
    Set,
    /// The vault could not be read; the reason never contains a key.
    Unreadable(String),
}

/// The AI settings of the company, as the AI and privacy screen shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSettings {
    /// The local model file and whether it is there.
    pub model_file: LocalModelFile,
    /// Whether the local model can answer (never, in this build: no runtime is linked).
    pub local_status: ProviderStatus,
    /// The cloud provider as described.
    pub cloud: CloudSettings,
    /// Whether the cloud key is saved.
    pub key: KeyState,
    /// The features switched off, in [`AiFeature::ALL`] order; every other feature is on.
    pub features_off: Vec<AiFeature>,
}

impl Api {
    /// Attaches the vault the cloud key is kept in for this session (the app attaches the OS
    /// keychain; tests and captures an in-memory vault). Attaching reads nothing.
    pub fn attach_key_vault(&mut self, vault: Box<dyn KeyVault>) {
        self.key_vault = Some(std::sync::Arc::from(vault));
    }

    /// The user's cloud provider as an assistant, sending through `transport` (the app passes
    /// HTTPS; tests a recording double). Making it reads the settings and whether a key is
    /// saved, and sends nothing; the key is read from the vault only when a request is made.
    ///
    /// # Errors
    /// [`ApiError::Store`] or [`ApiError::AiKeyAccount`] when the company cannot be read.
    pub fn cloud_assistant(
        &self,
        transport: Box<dyn CloudTransport>,
    ) -> Result<Outcome<CloudAssistant>, ApiError> {
        if let Some(refused) = self.cloud_refusal()? {
            return Ok(Outcome::Unsupported(refused));
        }
        let Some(vault) = self.key_vault.clone() else {
            return Ok(Outcome::Unsupported(no_vault()));
        };
        Ok(Outcome::Value(CloudAssistant {
            cloud: self.cloud_settings()?,
            account: self.key_account()?,
            vault,
            transport: std::sync::Arc::from(transport),
        }))
    }

    /// The AI settings stored for the company, the local model file as it is on disk now, and
    /// whether the cloud key is saved.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the audit log cannot be read; [`ApiError::AiKeyAccount`] when a
    /// vault is attached and the company cannot be read to name the key's account.
    pub fn ai_settings(&self) -> Result<AiSettings, ApiError> {
        let model = self.local_model()?;
        Ok(AiSettings {
            model_file: model.file(),
            local_status: model.status(),
            cloud: self.cloud_settings()?,
            key: self.key_state()?,
            features_off: AiFeature::ALL
                .into_iter()
                .map(|f| self.ai_feature_on(f).map(|on| (f, on)))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|(_, on)| !on)
                .map(|(f, _)| f)
                .collect(),
        })
    }

    /// Whether `feature` is switched on for this company. Every feature is on until switched
    /// off; the tier still decides whether any feature runs.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the audit log cannot be read.
    pub fn ai_feature_on(&self, feature: AiFeature) -> Result<bool, ApiError> {
        Ok(self.setting(feature_key(feature))?.as_deref() != Some(FEATURE_OFF))
    }

    /// Switches `feature` on or off for this company, as an audited setting.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the change cannot be committed.
    pub fn set_ai_feature(
        &mut self,
        feature: AiFeature,
        on: bool,
    ) -> Result<Outcome<()>, ApiError> {
        self.change_setting(
            feature_key(feature),
            if on { FEATURE_ON } else { FEATURE_OFF },
        )?;
        Ok(Outcome::Value(()))
    }

    /// The local model at the chosen file (or none chosen).
    ///
    /// # Errors
    /// [`ApiError::Store`] when the audit log cannot be read.
    pub fn local_model(&self) -> Result<LocalModel, ApiError> {
        let path = self
            .setting(LOCAL_MODEL_FILE_KEY)?
            .filter(|p| !p.is_empty())
            .map(PathBuf::from);
        Ok(LocalModel::new(path))
    }

    /// Stores the local model file (or none) as an audited setting. Only the path is stored.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the change cannot be committed.
    pub fn set_local_model_file(&mut self, path: Option<&Path>) -> Result<Outcome<()>, ApiError> {
        let text = match path {
            None => String::new(),
            Some(p) => match p.to_str() {
                Some(t) => t.to_owned(),
                None => {
                    return Ok(Outcome::Unsupported(Unsupported {
                        what: "this model file path".to_owned(),
                        reason: "the path is not valid Unicode text, so it cannot be stored \
                                 exactly"
                            .to_owned(),
                    }));
                }
            },
        };
        self.change_setting(LOCAL_MODEL_FILE_KEY, &text)?;
        Ok(Outcome::Value(()))
    }

    /// Stores the cloud provider as described (each part an audited setting).
    ///
    /// # Errors
    /// [`ApiError::Store`] when a change cannot be committed.
    pub fn set_cloud_settings(&mut self, cloud: &CloudSettings) -> Result<Outcome<()>, ApiError> {
        if let Some(refused) = cloud_settings_refusal(cloud) {
            return Ok(Outcome::Unsupported(refused));
        }
        for (key, value) in [
            (CLOUD_ENDPOINT_KEY, &cloud.endpoint),
            (CLOUD_FORMAT_KEY, &cloud.format),
            (CLOUD_MODEL_KEY, &cloud.model),
            (CLOUD_BILL_MODEL_KEY, &cloud.bill_model),
            (CLOUD_WORDS_MODEL_KEY, &cloud.words_model),
        ] {
            self.change_setting(key, value.as_deref().unwrap_or(""))?;
        }
        Ok(Outcome::Value(()))
    }

    /// Saves the cloud key in the attached vault. Nothing goes into the company file.
    ///
    /// # Errors
    /// [`ApiError::Store`] or [`ApiError::AiKeyAccount`] when the company cannot be read to name
    /// the key's account.
    pub fn save_cloud_key(&mut self, key: &str) -> Result<Outcome<()>, ApiError> {
        if key.trim().is_empty() {
            return Ok(Outcome::Unsupported(Unsupported {
                what: "saving the cloud key".to_owned(),
                reason: "the key is empty".to_owned(),
            }));
        }
        let account = self.key_account()?;
        let Some(vault) = self.key_vault.as_ref() else {
            return Ok(Outcome::Unsupported(no_vault()));
        };
        Ok(match vault.save_key(KEY_SERVICE, &account, key) {
            Ok(()) => Outcome::Value(()),
            Err(e) => Outcome::Unsupported(Unsupported {
                what: "saving the cloud key".to_owned(),
                reason: e.to_string(),
            }),
        })
    }

    /// Deletes the cloud key from the attached vault.
    ///
    /// # Errors
    /// [`ApiError::Store`] or [`ApiError::AiKeyAccount`] when the company cannot be read to name
    /// the key's account.
    pub fn delete_cloud_key(&mut self) -> Result<Outcome<()>, ApiError> {
        let account = self.key_account()?;
        let Some(vault) = self.key_vault.as_ref() else {
            return Ok(Outcome::Unsupported(no_vault()));
        };
        Ok(match vault.delete_key(KEY_SERVICE, &account) {
            Ok(()) => Outcome::Value(()),
            Err(e) => Outcome::Unsupported(Unsupported {
                what: "deleting the cloud key".to_owned(),
                reason: e.to_string(),
            }),
        })
    }

    /// Why the cloud tier cannot be chosen now, or `None` when it can.
    pub(crate) fn cloud_refusal(&self) -> Result<Option<Unsupported>, ApiError> {
        let what = "AI cloud tier".to_owned();
        if self.key_vault.is_none() {
            return Ok(Some(Unsupported {
                what,
                reason: "the provider key must be saved in the OS keychain, and no keychain \
                         store is attached; the company file never holds a key"
                    .to_owned(),
            }));
        }
        let cloud = self.cloud_settings()?;
        let mut missing: Vec<&str> = Vec::new();
        if cloud.format.is_none() {
            missing.push("the request format");
        }
        if cloud_request_url(&cloud).is_none() {
            missing.push("the endpoint");
        }
        if !missing.is_empty() {
            return Ok(Some(Unsupported {
                what,
                reason: format!("the provider is not described yet: {}", missing.join(", ")),
            }));
        }
        Ok(match self.key_state()? {
            KeyState::Set => None,
            KeyState::NoVault | KeyState::NotSet => Some(Unsupported {
                what,
                reason: "no key is saved for this company in the OS keychain".to_owned(),
            }),
            KeyState::Unreadable(reason) => Some(Unsupported {
                what,
                reason: format!("the key could not be read: {reason}"),
            }),
        })
    }

    fn cloud_settings(&self) -> Result<CloudSettings, ApiError> {
        let read = |key| {
            self.setting(key)
                .map(|v| v.filter(|text: &String| !text.is_empty()))
        };
        Ok(CloudSettings {
            endpoint: read(CLOUD_ENDPOINT_KEY)?,
            format: read(CLOUD_FORMAT_KEY)?,
            model: read(CLOUD_MODEL_KEY)?,
            bill_model: read(CLOUD_BILL_MODEL_KEY)?,
            words_model: read(CLOUD_WORDS_MODEL_KEY)?,
        })
    }

    fn key_state(&self) -> Result<KeyState, ApiError> {
        let Some(vault) = self.key_vault.as_ref() else {
            return Ok(KeyState::NoVault);
        };
        let account = self.key_account()?;
        Ok(match vault.has_key(KEY_SERVICE, &account) {
            Ok(true) => KeyState::Set,
            Ok(false) => KeyState::NotSet,
            Err(e) => KeyState::Unreadable(e.to_string()),
        })
    }

    /// The keychain account of this company's key: its id and name, so two companies keep two
    /// keys.
    fn key_account(&self) -> Result<String, ApiError> {
        let company = match self.books()? {
            Outcome::Value(books) => books.company,
            Outcome::Unsupported(u) => {
                return Err(ApiError::AiKeyAccount { reason: u.reason });
            }
        };
        Ok(format!("company-{}-{}", company.id.0, company.name))
    }

    /// The last stored value of setting `key`, or `None` when it was never set.
    fn setting(&self, key: &str) -> Result<Option<String>, ApiError> {
        let records = read_all(self.store())?;
        Ok(records.iter().rev().find_map(|record| match &record.event {
            Event::SettingChanged { key: k, after, .. } if k == key => Some(after.clone()),
            Event::SettingChanged { .. }
            | Event::MasterCreated(_)
            | Event::MasterAltered { .. }
            | Event::MasterDeleted(_)
            | Event::VoucherPosted(_)
            | Event::VoucherAltered { .. }
            | Event::VoucherCancelled { .. }
            | Event::VoucherDeleted { .. }
            | Event::PeriodLocked { .. }
            | Event::StatementImported(_)
            | Event::MatchConfirmed(_)
            | Event::AiProposalRecorded(_)
            | Event::BankStatementStored { .. }
            | Event::Gstr2bImported { .. }
            | Event::ImsActionRecorded { .. }
            | Event::Gstr3bFiled { .. }
            | Event::AiActivityRecorded(_) => None,
        }))
    }

    /// Stores `value` under setting `key` as an audited change; the same value writes nothing.
    fn change_setting(&mut self, key: &str, value: &str) -> Result<(), ApiError> {
        let before = self.setting(key)?.unwrap_or_default();
        if before == value {
            return Ok(());
        }
        let event = Event::SettingChanged {
            key: key.to_owned(),
            before,
            after: value.to_owned(),
        };
        let actor = self.actor().clone();
        self.commit_as(&[event], &actor)?;
        Ok(())
    }
}

/// The stored word of a feature switched on.
const FEATURE_ON: &str = "on";
/// The stored word of a feature switched off.
const FEATURE_OFF: &str = "off";

/// The setting key of a feature's switch.
fn feature_key(feature: AiFeature) -> &'static str {
    match feature {
        AiFeature::BillReading => "ai.feature.bill_reading",
        AiFeature::PlainWords => "ai.feature.plain_words",
        AiFeature::BankSuggestion => "ai.feature.bank_suggestion",
        AiFeature::Ask => "ai.feature.ask",
        AiFeature::Gstr2bExplanation => "ai.feature.gstr2b_explanation",
    }
}

fn no_vault() -> Unsupported {
    Unsupported {
        what: "the cloud key".to_owned(),
        reason: "no keychain store is attached, so a key cannot be saved; the company file never \
                 holds a key"
            .to_owned(),
    }
}

/// The user's cloud provider (bring your own key), one model per feature as
/// [`cloud_model_for`] names it. Each request is built by the cloud client: redacted, https
/// only, the key from the vault in a header. A document is never sent.
pub struct CloudAssistant {
    cloud: CloudSettings,
    account: String,
    vault: Arc<dyn KeyVault>,
    transport: Arc<dyn CloudTransport>,
}

impl std::fmt::Debug for CloudAssistant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudAssistant")
            .field("cloud", &self.cloud)
            .finish_non_exhaustive()
    }
}

/// The vault as the client's key store.
struct VaultKeys(Arc<dyn KeyVault>);

impl KeyStore for VaultKeys {
    fn api_key(&self, service: &str, account: &str) -> Result<String, AssistError> {
        self.0.api_key(service, account)
    }
}

/// The shared transport as the client's own.
struct SharedTransport(Arc<dyn CloudTransport>);

impl CloudTransport for SharedTransport {
    fn post(&self, request: &CloudRequest) -> Result<CloudResponse, AssistError> {
        self.0.post(request)
    }
}

impl CloudAssistant {
    /// The provider for `feature`, or why there is none.
    fn model(&self, feature: AiFeature) -> Result<CloudModel, AssistError> {
        let unsupported = |reason: String| {
            AssistError::Unsupported(Unsupported {
                what: "the cloud provider".to_owned(),
                reason,
            })
        };
        let provider = match self.cloud.format.as_deref() {
            Some(f) if f == CLOUD_FORMATS[1] => CloudProvider::Anthropic,
            Some(f) if f == CLOUD_FORMATS[0] => CloudProvider::OpenAiCompatible,
            Some(_) | None => return Err(unsupported("no request format is chosen".to_owned())),
        };
        let endpoint = cloud_request_url(&self.cloud)
            .ok_or_else(|| unsupported("no endpoint is given".to_owned()))?;
        let model = cloud_model_for(&self.cloud, feature).ok_or_else(|| {
            unsupported(format!("no model is named for {}", feature_words(feature)))
        })?;
        Ok(CloudModel::new(CloudClient::new(
            CloudConfig {
                provider,
                endpoint,
                model,
                key_service: KEY_SERVICE.to_owned(),
                key_account: self.account.clone(),
                max_output_tokens: MAX_OUTPUT_TOKENS,
            },
            Box::new(VaultKeys(Arc::clone(&self.vault))),
            Box::new(SharedTransport(Arc::clone(&self.transport))),
        )))
    }
}

/// A feature in the words of a refusal.
fn feature_words(feature: AiFeature) -> &'static str {
    match feature {
        AiFeature::BillReading => "bill reading",
        AiFeature::PlainWords => "plain words",
        AiFeature::BankSuggestion => "bank suggestions",
        AiFeature::Ask => "questions about the books",
        AiFeature::Gstr2bExplanation => "2B explanations",
    }
}

impl AiProvider for CloudAssistant {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: self
                .cloud
                .format
                .clone()
                .unwrap_or_else(|| "cloud".to_owned()),
            version: ledgerly_assist::cloud::PROMPT_TEMPLATE_VERSION.to_owned(),
            place: ProviderPlace::Cloud,
        }
    }

    fn status(&self) -> ProviderStatus {
        ProviderStatus::Ready
    }

    fn read_text(
        &self,
        text: &str,
        ledgers: &[MasterLedger],
        cancel: &CancelToken,
    ) -> Result<Proposal, AssistError> {
        self.model(AiFeature::PlainWords)?
            .read_text(text, ledgers, cancel)
    }

    fn ocr(
        &self,
        _bytes: &[u8],
        _sha256: &str,
        _cancel: &CancelToken,
    ) -> Result<ledgerly_assist::ocr::OcrOutput, AssistError> {
        Err(AssistError::Unsupported(Unsupported {
            what: "reading a bill in the cloud".to_owned(),
            reason: "an image cannot be redacted before it leaves this computer, and the \
                     per-document confirmation is not built"
                .to_owned(),
        }))
    }

    fn ask(
        &self,
        question: &str,
        cancel: &CancelToken,
    ) -> Result<ledgerly_assist::ReportQuery, AssistError> {
        self.model(AiFeature::Ask)?.ask(question, cancel)
    }
}
