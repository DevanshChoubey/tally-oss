//! AI settings beyond the tier (round s21, plan round B; spec 6.8, 9): the local model file, the
//! cloud provider the user describes, and the cloud key.
//!
//! The model file and the provider are audited settings in the company file (`SettingChanged`
//! under [`LOCAL_MODEL_FILE_KEY`] and `ai.cloud.*`). The key is never in the company file: it is
//! kept in the attached [`KeyVault`] (the OS keychain in the app) under [`KEY_SERVICE`] and an
//! account named after the company. Reading these settings touches the vault only when one is
//! attached, and never the network.

use std::path::{Path, PathBuf};

use ledgerly_assist::cloud::KeyVault;
use ledgerly_assist::provider::{AiProvider, LocalModel, LocalModelFile, ProviderStatus};
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
/// The setting key of the cloud model name.
const CLOUD_MODEL_KEY: &str = "ai.cloud.model";
/// The keychain service the cloud key is saved under.
pub const KEY_SERVICE: &str = "ledgerly-ai-cloud";

/// The cloud provider as the user described it. Every part is the user's choice (bring your own
/// key); none has a default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CloudSettings {
    /// The endpoint URL.
    pub endpoint: Option<String>,
    /// The request format, as the provider documents it (`chat-completions` or `messages`).
    pub format: Option<String>,
    /// The model name the provider expects.
    pub model: Option<String>,
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
        self.key_vault = Some(vault);
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
        for (key, value) in [
            (CLOUD_ENDPOINT_KEY, &cloud.endpoint),
            (CLOUD_FORMAT_KEY, &cloud.format),
            (CLOUD_MODEL_KEY, &cloud.model),
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
        let missing: Vec<&str> = [
            ("the endpoint", &cloud.endpoint),
            ("the request format", &cloud.format),
            ("the model name", &cloud.model),
        ]
        .into_iter()
        .filter(|(_, v)| v.is_none())
        .map(|(name, _)| name)
        .collect();
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
