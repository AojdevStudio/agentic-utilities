//! Value writes through the Bitwarden Secrets Manager SDK. `bws` takes write
//! values in argv, where other local users can read them, so the daemon never
//! uses it to create a secret or replace a value. No SDK error text leaves
//! this module, so a failure can never echo a value.

use super::backend::BWS_TIMEOUT;
use super::fields::{check_key, check_note, check_value};
use super::material::SecretMaterial;
use super::meta::{NewSecret, SecretMeta, SecretUpdate, WriteFailure};
use super::request::{RequestControl, WriteAttempt};
use bitwarden::secrets_manager::secrets::{SecretCreateRequest, SecretPutRequest, SecretResponse};
use bitwarden::secrets_manager::{
    AccessTokenLoginRequest, ClientSettings, DeviceType, SecretsManagerClient,
};
use secrecy::{ExposeSecret, SecretString};
use std::fmt::Debug;
use std::future::Future;
use std::os::fd::BorrowedFd;
use std::task::Poll;
use tokio::io::unix::AsyncFd;
use tokio::io::Interest;
use tokio::runtime::{Builder, Runtime};
use uuid::Uuid;
use zeroize::Zeroize;
use WriteFailure::{Rejected, Stopped, Uncertain};

/// One SDK login per write. Writes are rare human actions, so no session is cached.
pub(crate) struct SdkWriter {
    token: SecretString,
    identity_url: String,
    api_url: String,
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
        }
    }

    /// Sends exactly one create. Fields the SDK would refuse never reach a
    /// login. A failure after the request may have reached the server is
    /// `Uncertain` and is never retried.
    pub(crate) fn create(
        &self,
        secret: &NewSecret,
        value: &SecretMaterial,
        attempt: &mut WriteAttempt<'_>,
    ) -> Result<SecretMeta, WriteFailure> {
        valid(&secret.key, &secret.note, value)?;
        let project = Uuid::parse_str(&secret.project_id).map_err(|_| Rejected)?;
        let (runtime, client, organization_id) = self.connect(attempt.control())?;
        let request = Wiped(
            SecretCreateRequest {
                organization_id,
                key: secret.key.clone(),
                value: utf8(value)?,
                note: secret.note.clone(),
                project_ids: Some(vec![project]),
            },
            |request| request.value.zeroize(),
        );
        let secrets = client.secrets();
        attempt.begin_remote()?;
        let outcome = runtime.block_on(watched(attempt.control(), secrets.create(&request.0)));
        finish(outcome)
    }

    /// Writes the full record once: key, value, note, and project.
    pub(crate) fn update(
        &self,
        secret: &SecretUpdate,
        value: &SecretMaterial,
        attempt: &mut WriteAttempt<'_>,
    ) -> Result<SecretMeta, WriteFailure> {
        valid(&secret.key, &secret.note, value)?;
        let id = Uuid::parse_str(&secret.id).map_err(|_| Rejected)?;
        let project_ids = match secret.project_id.as_deref() {
            Some(project) => Some(vec![Uuid::parse_str(project).map_err(|_| Rejected)?]),
            None => None,
        };
        let (runtime, client, organization_id) = self.connect(attempt.control())?;
        let request = Wiped(
            SecretPutRequest {
                id,
                organization_id,
                key: secret.key.clone(),
                value: utf8(value)?,
                note: secret.note.clone(),
                project_ids,
            },
            |request| request.value.zeroize(),
        );
        let secrets = client.secrets();
        attempt.begin_remote()?;
        let outcome = runtime.block_on(watched(attempt.control(), secrets.update(&request.0)));
        finish(outcome)
    }

    /// Log in with the daemon token. A failed login wrote nothing. A login
    /// the client abandoned, or that outlived the request, is `Stopped`.
    /// Name lookups are tasks on this runtime, so dropping it after a stop
    /// leaves none behind and waits for none.
    fn connect(
        &self,
        control: &RequestControl<'_>,
    ) -> Result<(Runtime, SecretsManagerClient, Uuid), WriteFailure> {
        control.check().map_err(|_| Stopped)?;
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Rejected)?;
        let client = {
            let _context = runtime.enter();
            SecretsManagerClient::new(Some(self.settings()))
        };
        let request = Wiped(
            AccessTokenLoginRequest {
                access_token: self.token.expose_secret().to_owned(),
                state_file: None,
            },
            |request| request.access_token.zeroize(),
        );
        let auth = client.auth();
        let login = runtime.block_on(watched(control, auth.login_access_token(&request.0)));
        drop(request);
        match login {
            _ if control.check().is_err() => return Err(Stopped),
            Some(Ok(response)) if response.authenticated => {}
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

/// Fields the SDK would refuse. It copies a refused field into its error and
/// debug log, so they never reach a login or a request object.
fn valid(key: &str, note: &str, value: &SecretMaterial) -> Result<(), WriteFailure> {
    check_key(key)
        .and(check_note(note))
        .and(check_value(value.expose_bytes()))
        .map_err(|_| Rejected)
}

/// An SDK request whose secret fields are wiped on every exit path.
struct Wiped<T>(T, fn(&mut T));

impl<T> Drop for Wiped<T> {
    fn drop(&mut self) {
        (self.1)(&mut self.0);
    }
}

/// Run one SDK call until it finishes, its own limit or the request deadline
/// passes, or the client leaves. `None` means it was abandoned unfinished.
/// A finished call wins over a simultaneous stop, so a definitive provider
/// answer is never thrown away.
async fn watched<F: Future>(control: &RequestControl<'_>, call: F) -> Option<F::Output> {
    let deadline = tokio::time::Instant::from_std(control.deadline(BWS_TIMEOUT));
    let mut call = std::pin::pin!(tokio::time::timeout_at(deadline, call));
    let mut left = std::pin::pin!(client_left(control.socket()));
    std::future::poll_fn(|context| {
        if let Poll::Ready(done) = call.as_mut().poll(context) {
            return Poll::Ready(done.ok());
        }
        left.as_mut().poll(context).map(|()| None)
    })
    .await
}

/// Ends when the client socket is readable: a hangup or a protocol break.
/// A socket that cannot be watched ends it at once, so no call runs unwatched.
async fn client_left(socket: Option<BorrowedFd<'_>>) {
    let Some(socket) = socket else {
        return std::future::pending().await;
    };
    // SAFETY: the borrow keeps the socket open, and the same descriptor, for
    // as long as the registration that lives inside this future.
    if let Ok(watch) = unsafe { AsyncFd::register_with_interest(socket, Interest::READABLE) } {
        // Readiness and a failed wait both mean the request must stop.
        let _ready = watch.readable().await;
    }
}

fn finish<E: Debug>(
    outcome: Option<Result<SecretResponse, E>>,
) -> Result<SecretMeta, WriteFailure> {
    match outcome {
        Some(Ok(response)) => Ok(meta(response)),
        Some(Err(error)) => Err(classify(&error)),
        None => Err(Uncertain),
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
