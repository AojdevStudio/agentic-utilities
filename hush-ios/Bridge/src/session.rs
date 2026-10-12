//! One SDK client, one async runtime, and the lock state for a session.

use std::io;
use std::time::Duration;

use bitwarden::secrets_manager::projects::ProjectsListRequest;
use bitwarden::secrets_manager::secrets::{
    SecretCreateRequest, SecretGetRequest, SecretIdentifiersRequest, SecretResponse,
};
use bitwarden::secrets_manager::{
    AccessToken, AccessTokenLoginRequest, ClientSettings, DeviceType, SecretsManagerClient,
};
use tokio::runtime::{Builder, Runtime};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::classify::{Failure, create_code, failure, login_code, read_code};
use crate::request::{NewSecret, Request};
use crate::response::{
    Catalog, Data, ErrorCode, LoginData, ProjectSummary, SecretDetail, SecretSummary,
};

/// Client settings for region 0 (US cloud) or 1 (EU cloud).
pub fn settings_for(region: u32) -> Option<ClientSettings> {
    let (identity, api) = match region {
        0 => (
            "https://identity.bitwarden.com",
            "https://api.bitwarden.com",
        ),
        1 => (
            "https://vault.bitwarden.eu/identity",
            "https://vault.bitwarden.eu/api",
        ),
        _ => return None,
    };
    Some(settings(identity, api))
}

/// Client settings for explicit endpoints.
pub fn settings(identity_url: &str, api_url: &str) -> ClientSettings {
    ClientSettings {
        identity_url: identity_url.to_owned(),
        api_url: api_url.to_owned(),
        user_agent: "Hush iOS".to_owned(),
        device_type: DeviceType::SDK,
        device_identifier: None,
        bitwarden_client_version: None,
        bitwarden_package_type: None,
    }
}

/// A session starts locked. A successful login stores the organization.
pub struct Session {
    runtime: Runtime,
    client: SecretsManagerClient,
    organization: Option<Uuid>,
    deadline: Duration,
}

impl Session {
    pub fn new(settings: ClientSettings, deadline: Duration) -> io::Result<Self> {
        let runtime = Builder::new_current_thread().enable_all().build()?;
        let client = {
            let _context = runtime.enter();
            SecretsManagerClient::new(Some(settings))
        };
        Ok(Self {
            runtime,
            client,
            organization: None,
            deadline,
        })
    }

    /// Runs one validated request. Data operations need a prior login.
    pub fn handle(&mut self, request: Request) -> Result<Data, ErrorCode> {
        match (request, self.organization) {
            (Request::Login { token }, None) => self.login(&token).map_err(login_code),
            (Request::Login { .. }, Some(_)) => Err(ErrorCode::InvalidRequest),
            (_, None) => Err(ErrorCode::Locked),
            (Request::Catalog, Some(organization)) => self.catalog(organization).map_err(read_code),
            (Request::Read { id }, Some(_)) => self.read(id).map_err(read_code),
            (Request::Create(secret), Some(organization)) => {
                self.create(organization, secret).map_err(create_code)
            }
        }
    }

    /// Checks the token shape locally, then exchanges it with the identity server.
    fn login(&mut self, token: &str) -> Result<Data, Failure> {
        token
            .parse::<AccessToken>()
            .map_err(|_| Failure::BadToken)?;
        let mut request = AccessTokenLoginRequest {
            access_token: token.to_owned(),
            state_file: None,
        };
        let auth = self.client.auth();
        let outcome = self.run(async {
            let response = auth.login_access_token(&request).await;
            response.map_err(|error| failure(&error))
        });
        request.access_token.zeroize();
        if !outcome?.authenticated {
            return Err(Failure::IdentityRejected);
        }
        let organization: Uuid = self
            .client
            .get_access_token_organization()
            .ok_or(Failure::Unexpected)?
            .into();
        self.organization = Some(organization);
        Ok(Data::Login(LoginData {
            organization_id: organization,
        }))
    }

    /// Lists project and secret metadata. Never fetches values or notes.
    fn catalog(&self, organization_id: Uuid) -> Result<Data, Failure> {
        let (projects, secrets) = (self.client.projects(), self.client.secrets());
        let project_request = ProjectsListRequest { organization_id };
        let secret_request = SecretIdentifiersRequest { organization_id };
        let (projects, secrets) = self.run(async {
            let projects = projects.list(&project_request).await;
            let projects = projects.map_err(|error| failure(&error))?;
            let secrets = secrets.list(&secret_request).await;
            let secrets = secrets.map_err(|error| failure(&error))?;
            Ok((projects, secrets))
        })?;
        let projects = projects.data.into_iter();
        let secrets = secrets.data.into_iter();
        Ok(Data::Catalog(Catalog {
            projects: projects
                .map(|project| ProjectSummary {
                    id: project.id,
                    name: project.name,
                })
                .collect(),
            secrets: secrets
                .map(|secret| SecretSummary {
                    id: secret.id,
                    key: secret.key,
                    project_ids: secret.project_ids,
                })
                .collect(),
        }))
    }

    fn read(&self, id: Uuid) -> Result<Data, Failure> {
        let secrets = self.client.secrets();
        let secret = self.run(async {
            let response = secrets.get(&SecretGetRequest { id }).await;
            response.map_err(|error| failure(&error))
        })?;
        Ok(Data::Secret(detail(secret)))
    }

    /// Sends exactly one create. The SDK response is the round trip proof.
    fn create(&self, organization_id: Uuid, mut secret: NewSecret) -> Result<Data, Failure> {
        let mut request = SecretCreateRequest {
            organization_id,
            key: secret.key,
            value: std::mem::take(&mut *secret.value),
            note: std::mem::take(&mut *secret.note),
            project_ids: Some(vec![secret.project_id]),
        };
        let secrets = self.client.secrets();
        let outcome = self.run(async {
            let response = secrets.create(&request).await;
            response.map_err(|error| failure(&error))
        });
        request.value.zeroize();
        request.note.zeroize();
        Ok(Data::Secret(detail(outcome?)))
    }

    /// Runs SDK work on the session runtime under the session deadline.
    fn run<T>(&self, work: impl Future<Output = Result<T, Failure>>) -> Result<T, Failure> {
        self.runtime.block_on(async {
            let outcome = tokio::time::timeout(self.deadline, work).await;
            outcome.unwrap_or(Err(Failure::Timeout))
        })
    }
}

/// Moves an SDK secret into the zeroizing response shape.
fn detail(secret: SecretResponse) -> SecretDetail {
    let SecretResponse {
        id,
        project_id,
        key,
        value,
        note,
        ..
    } = secret;
    SecretDetail {
        id,
        key,
        value: Zeroizing::new(value),
        note: Zeroizing::new(note),
        project_id,
    }
}
