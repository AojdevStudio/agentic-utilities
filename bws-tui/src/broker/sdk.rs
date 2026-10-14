//! Value writes through the Bitwarden Secrets Manager SDK. `bws` takes write
//! values in argv, where other local users can read them, so the daemon never
//! uses it to create a secret or replace a value. No SDK error text leaves
//! this module, so a failure can never echo a value.

use super::backend::BWS_TIMEOUT;
use super::material::SecretMaterial;
use super::meta::{NewSecret, SecretMeta, SecretUpdate, WriteFailure};
use bitwarden::secrets_manager::secrets::{SecretCreateRequest, SecretPutRequest, SecretResponse};
use bitwarden::secrets_manager::{
    AccessTokenLoginRequest, ClientSettings, DeviceType, SecretsManagerClient,
};
use secrecy::{ExposeSecret, SecretString};
use std::fmt::Debug;
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};
use uuid::Uuid;
use zeroize::Zeroize;
use WriteFailure::{Rejected, Uncertain};

/// One SDK login per write. Writes are rare human actions, so no session is cached.
pub(crate) struct SdkWriter {
    token: SecretString,
    identity_url: String,
    api_url: String,
    deadline: Duration,
}

impl SdkWriter {
    /// The US cloud, which is also what `bws` uses with no server configured.
    pub(crate) fn cloud(token: SecretString) -> Self {
        Self::new(
            token,
            "https://identity.bitwarden.com",
            "https://api.bitwarden.com",
        )
    }

    pub(crate) fn new(token: SecretString, identity_url: &str, api_url: &str) -> Self {
        Self {
            token,
            identity_url: identity_url.to_owned(),
            api_url: api_url.to_owned(),
            deadline: BWS_TIMEOUT,
        }
    }

    /// Sends exactly one create. A failure after the request may have reached
    /// the server is `Uncertain` and is never retried.
    pub(crate) fn create(
        &self,
        secret: &NewSecret,
        value: &SecretMaterial,
    ) -> Result<SecretMeta, WriteFailure> {
        let project = Uuid::parse_str(&secret.project_id).map_err(|_| Rejected)?;
        let (runtime, client, organization_id) = self.connect()?;
        let mut request = SecretCreateRequest {
            organization_id,
            key: secret.key.clone(),
            value: utf8(value)?,
            note: secret.note.clone(),
            project_ids: Some(vec![project]),
        };
        let secrets = client.secrets();
        let outcome = runtime.block_on(async {
            tokio::time::timeout(self.deadline, secrets.create(&request)).await
        });
        request.value.zeroize();
        finish(outcome)
    }

    /// Writes the full record once: key, value, note, and project.
    pub(crate) fn update(
        &self,
        secret: &SecretUpdate,
        value: &SecretMaterial,
    ) -> Result<SecretMeta, WriteFailure> {
        let id = Uuid::parse_str(&secret.id).map_err(|_| Rejected)?;
        let project_ids = match secret.project_id.as_deref() {
            Some(project) => Some(vec![Uuid::parse_str(project).map_err(|_| Rejected)?]),
            None => None,
        };
        let (runtime, client, organization_id) = self.connect()?;
        let mut request = SecretPutRequest {
            id,
            organization_id,
            key: secret.key.clone(),
            value: utf8(value)?,
            note: secret.note.clone(),
            project_ids,
        };
        let secrets = client.secrets();
        let outcome = runtime.block_on(async {
            tokio::time::timeout(self.deadline, secrets.update(&request)).await
        });
        request.value.zeroize();
        finish(outcome)
    }

    /// Log in with the daemon token. A failed login wrote nothing.
    fn connect(&self) -> Result<(Runtime, SecretsManagerClient, Uuid), WriteFailure> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Rejected)?;
        let client = {
            let _context = runtime.enter();
            SecretsManagerClient::new(Some(self.settings()))
        };
        let mut request = AccessTokenLoginRequest {
            access_token: self.token.expose_secret().to_owned(),
            state_file: None,
        };
        let auth = client.auth();
        let login = runtime.block_on(async {
            tokio::time::timeout(self.deadline, auth.login_access_token(&request)).await
        });
        request.access_token.zeroize();
        match login {
            Ok(Ok(response)) if response.authenticated => {}
            _ => return Err(Rejected),
        }
        let organization: Uuid = client
            .get_access_token_organization()
            .ok_or(Rejected)?
            .into();
        Ok((runtime, client, organization))
    }

    fn settings(&self) -> ClientSettings {
        ClientSettings {
            identity_url: self.identity_url.clone(),
            api_url: self.api_url.clone(),
            user_agent: "hush daemon".to_owned(),
            device_type: DeviceType::SDK,
            device_identifier: None,
            bitwarden_client_version: None,
            bitwarden_package_type: None,
        }
    }
}

/// The SDK takes values as text. Non-UTF-8 bytes are refused before any call.
fn utf8(value: &SecretMaterial) -> Result<String, WriteFailure> {
    String::from_utf8(value.expose_bytes().to_vec()).map_err(|error| {
        error.into_bytes().zeroize();
        Rejected
    })
}

fn finish<E: Debug, T>(
    outcome: Result<Result<SecretResponse, E>, T>,
) -> Result<SecretMeta, WriteFailure> {
    match outcome {
        Ok(Ok(response)) => Ok(meta(response)),
        Ok(Err(error)) => Err(classify(&error)),
        Err(_) => Err(Uncertain),
    }
}

/// Validation and 4xx answers wrote nothing. Every other failure is uncertain.
/// The SDK error type is private, so only the `Debug` prefix is read, then wiped.
fn classify(error: &impl Debug) -> WriteFailure {
    let mut text = format!("{error:?}");
    let failure = if text.starts_with("Validation(") {
        Rejected
    } else if let Some(rest) = text.strip_prefix("Api(ResponseContent { status: ") {
        match rest.get(..3).and_then(|digits| digits.parse::<u16>().ok()) {
            Some(400..=499) => Rejected,
            _ => Uncertain,
        }
    } else {
        Uncertain
    };
    text.zeroize();
    failure
}

fn meta(response: SecretResponse) -> SecretMeta {
    let SecretResponse {
        id,
        key,
        mut value,
        note,
        project_id,
        ..
    } = response;
    value.zeroize();
    SecretMeta {
        id: id.to_string(),
        key,
        project: None,
        project_id: project_id.map(|project| project.to_string()),
        note,
    }
}
