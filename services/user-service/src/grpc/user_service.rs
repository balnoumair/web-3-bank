//! gRPC handler implementation for the UserService.
//!
//! Bridges proto request/response types to the domain layer.
//! All business rule validation (address format, aggregate root
//! methods) is delegated to domain entities before any repository
//! call is made.

use std::sync::Arc;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::domain::entities::Credential as DomainCredential;
use crate::domain::errors::DomainError;
use crate::domain::repository::{CredentialRepository, UserRepository};
use crate::domain::validation::{TempoAddress, Username};
use crate::grpc::{
    user_service_server::UserService, AddCredentialRequest, AddCredentialResponse,
    CreateUserRequest, CreateUserResponse, Credential, GetUserByAddressRequest,
    GetUserByAddressResponse, GetUserByCredentialIdRequest, GetUserByCredentialIdResponse,
    GetUserByUsernameRequest, GetUserByUsernameResponse, GetUserHomeChainRequest,
    GetUserHomeChainResponse, ListCredentialsRequest, ListCredentialsResponse,
    RevokeCredentialRequest, RevokeCredentialResponse, SetUserHomeChainRequest,
    SetUserHomeChainResponse, SetUsernameRequest, SetUsernameResponse, UpdateUserRequest,
    UpdateUserResponse,
};

fn domain_err_to_status(e: DomainError) -> Status {
    match e {
        DomainError::LastActiveCredential => {
            Status::failed_precondition("cannot revoke the last active credential")
        }
        DomainError::CredentialNotFound => {
            Status::not_found("credential not found or already revoked")
        }
        DomainError::UserNotFound => Status::not_found("user not found"),
        DomainError::AlreadyExists => {
            Status::already_exists("address or credential already registered")
        }
        DomainError::InvalidTempoAddress => {
            Status::invalid_argument("tempo_address must be a 0x-prefixed 40-char hex string")
        }
        DomainError::InvalidUsername => Status::invalid_argument(
            "username must be 3-20 chars, start with a letter, alphanumeric/underscore only",
        ),
        DomainError::UsernameTaken => Status::already_exists("username already taken"),
        DomainError::Infrastructure(msg) => Status::internal(msg),
    }
}

pub struct UserServiceImpl {
    pub user_repo: Arc<dyn UserRepository>,
    pub credential_repo: Arc<dyn CredentialRepository>,
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn create_user(
        &self,
        request: Request<CreateUserRequest>,
    ) -> Result<Response<CreateUserResponse>, Status> {
        let req = request.into_inner();

        // Validate credential invariants via the domain constructor before
        // touching the database.
        let credential = DomainCredential::new(
            uuid::Uuid::nil(), // placeholder; actual user_id assigned after user insert
            req.credential_id.clone(),
            req.public_key.clone(),
            req.tempo_address.clone(),
        )
        .map_err(domain_err_to_status)?;

        let user_id = self
            .user_repo
            .insert(req.display_name.as_deref())
            .await
            .map_err(domain_err_to_status)?;

        self.credential_repo
            .insert(
                user_id,
                &credential.credential_id,
                &credential.public_key,
                &credential.tempo_address, // already a TempoAddress
            )
            .await
            .map_err(domain_err_to_status)?;

        Ok(Response::new(CreateUserResponse {
            user_id: user_id.to_string(),
        }))
    }

    async fn get_user_by_address(
        &self,
        request: Request<GetUserByAddressRequest>,
    ) -> Result<Response<GetUserByAddressResponse>, Status> {
        let req = request.into_inner();

        let addr =
            TempoAddress::try_from(req.tempo_address.as_str()).map_err(domain_err_to_status)?;

        let row = self
            .credential_repo
            .get_user_by_address(&addr)
            .await
            .map_err(domain_err_to_status)?
            .ok_or_else(|| Status::not_found("user not found for given tempo_address"))?;

        Ok(Response::new(GetUserByAddressResponse {
            user_id: row.user_id.to_string(),
            display_name: row.display_name,
            status: row.status.to_string(),
            tempo_address: row.tempo_address.to_string(),
            username: row.username.map(|u| u.0).unwrap_or_default(),
        }))
    }

    async fn get_user_by_credential_id(
        &self,
        request: Request<GetUserByCredentialIdRequest>,
    ) -> Result<Response<GetUserByCredentialIdResponse>, Status> {
        let req = request.into_inner();

        let row = self
            .credential_repo
            .get_user_by_credential_id(&req.credential_id)
            .await
            .map_err(domain_err_to_status)?
            .ok_or_else(|| Status::not_found("user not found for given credential_id"))?;

        Ok(Response::new(GetUserByCredentialIdResponse {
            user_id: row.user_id.to_string(),
            display_name: row.display_name,
            status: row.status.to_string(),
            tempo_address: row.tempo_address.to_string(),
            username: row.username.map(|u| u.0).unwrap_or_default(),
        }))
    }

    async fn list_credentials(
        &self,
        request: Request<ListCredentialsRequest>,
    ) -> Result<Response<ListCredentialsResponse>, Status> {
        let req = request.into_inner();

        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|_| Status::invalid_argument("user_id must be a valid UUID"))?;

        let creds = self
            .credential_repo
            .list(user_id, false)
            .await
            .map_err(domain_err_to_status)?
            .into_iter()
            .map(|c| Credential {
                credential_id: URL_SAFE_NO_PAD.encode(&c.credential_id),
                tempo_address: c.tempo_address.to_string(),
                created_at: c.created_at.to_rfc3339(),
                revoked: c.revoked_at.is_some(),
            })
            .collect();

        Ok(Response::new(ListCredentialsResponse {
            credentials: creds,
        }))
    }

    async fn add_credential(
        &self,
        request: Request<AddCredentialRequest>,
    ) -> Result<Response<AddCredentialResponse>, Status> {
        let req = request.into_inner();

        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|_| Status::invalid_argument("user_id must be a valid UUID"))?;

        let user = self
            .user_repo
            .get_by_id(user_id)
            .await
            .map_err(domain_err_to_status)?
            .ok_or_else(|| Status::not_found("user not found"))?;

        // Route through the aggregate root — enforces address validation.
        let credential = user
            .add_credential(
                req.credential_id.clone(),
                req.public_key.clone(),
                req.tempo_address.clone(),
            )
            .map_err(domain_err_to_status)?;

        self.credential_repo
            .insert(
                user_id,
                &credential.credential_id,
                &credential.public_key,
                &credential.tempo_address,
            )
            .await
            .map_err(domain_err_to_status)?;

        let encoded_id = URL_SAFE_NO_PAD.encode(&req.credential_id);

        Ok(Response::new(AddCredentialResponse {
            credential_id: encoded_id,
        }))
    }

    async fn update_user(
        &self,
        request: Request<UpdateUserRequest>,
    ) -> Result<Response<UpdateUserResponse>, Status> {
        let req = request.into_inner();

        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|_| Status::invalid_argument("invalid user_id"))?;

        let user = self
            .user_repo
            .get_by_id(user_id)
            .await
            .map_err(domain_err_to_status)?
            .ok_or_else(|| Status::not_found("user not found"))?;

        if let Some(name) = &req.display_name {
            self.user_repo
                .update_display_name(user_id, name)
                .await
                .map_err(domain_err_to_status)?;

            let updated = self
                .user_repo
                .get_by_id(user_id)
                .await
                .map_err(domain_err_to_status)?
                .ok_or_else(|| Status::not_found("user not found"))?;

            return Ok(Response::new(UpdateUserResponse {
                user_id: updated.id.to_string(),
                display_name: updated.display_name,
                username: updated.username.map(|u| u.0).unwrap_or_default(),
            }));
        }

        Ok(Response::new(UpdateUserResponse {
            user_id: user.id.to_string(),
            display_name: user.display_name,
            username: user.username.map(|u| u.0).unwrap_or_default(),
        }))
    }

    async fn set_username(
        &self,
        request: Request<SetUsernameRequest>,
    ) -> Result<Response<SetUsernameResponse>, Status> {
        let req = request.into_inner();

        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|_| Status::invalid_argument("user_id must be a valid UUID"))?;

        // Validate format before touching the DB.
        let username = Username::try_from(req.username.as_str()).map_err(domain_err_to_status)?;

        // Confirm user exists.
        self.user_repo
            .get_by_id(user_id)
            .await
            .map_err(domain_err_to_status)?
            .ok_or_else(|| Status::not_found("user not found"))?;

        self.user_repo
            .set_username(user_id, &username)
            .await
            .map_err(domain_err_to_status)?;

        // Re-fetch via credential to return the full profile (including address).
        let row = self
            .credential_repo
            .get_user_by_username(&username)
            .await
            .map_err(domain_err_to_status)?
            .ok_or_else(|| Status::internal("user has no active credential after set_username"))?;

        Ok(Response::new(SetUsernameResponse {
            user_id: row.user_id.to_string(),
            display_name: row.display_name,
            status: row.status.to_string(),
            tempo_address: row.tempo_address.to_string(),
            username: row.username.map(|u| u.0).unwrap_or_default(),
        }))
    }

    async fn get_user_by_username(
        &self,
        request: Request<GetUserByUsernameRequest>,
    ) -> Result<Response<GetUserByUsernameResponse>, Status> {
        let req = request.into_inner();

        let username = Username::try_from(req.username.as_str()).map_err(domain_err_to_status)?;

        let row = self
            .credential_repo
            .get_user_by_username(&username)
            .await
            .map_err(domain_err_to_status)?
            .ok_or_else(|| Status::not_found("user not found for given username"))?;

        Ok(Response::new(GetUserByUsernameResponse {
            user_id: row.user_id.to_string(),
            display_name: row.display_name,
            status: row.status.to_string(),
            tempo_address: row.tempo_address.to_string(),
            username: row.username.map(|u| u.0).unwrap_or_default(),
        }))
    }

    async fn revoke_credential(
        &self,
        request: Request<RevokeCredentialRequest>,
    ) -> Result<Response<RevokeCredentialResponse>, Status> {
        let req = request.into_inner();

        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|_| Status::invalid_argument("user_id must be a valid UUID"))?;

        self.credential_repo
            .revoke(user_id, &req.credential_id)
            .await
            .map_err(domain_err_to_status)?;

        Ok(Response::new(RevokeCredentialResponse { success: true }))
    }

    async fn get_user_home_chain(
        &self,
        request: Request<GetUserHomeChainRequest>,
    ) -> Result<Response<GetUserHomeChainResponse>, Status> {
        let req = request.into_inner();
        let addr =
            TempoAddress::try_from(req.tempo_address.as_str()).map_err(domain_err_to_status)?;

        let home = self
            .user_repo
            .get_home_chain_by_tempo_address(&addr)
            .await
            .map_err(domain_err_to_status)?;

        match home {
            Some(chain_id) if chain_id >= 0 => Ok(Response::new(GetUserHomeChainResponse {
                found: true,
                chain_id: chain_id as u64,
            })),
            _ => Ok(Response::new(GetUserHomeChainResponse {
                found: false,
                chain_id: 0,
            })),
        }
    }

    async fn set_user_home_chain(
        &self,
        request: Request<SetUserHomeChainRequest>,
    ) -> Result<Response<SetUserHomeChainResponse>, Status> {
        let req = request.into_inner();
        let addr =
            TempoAddress::try_from(req.tempo_address.as_str()).map_err(domain_err_to_status)?;

        self.user_repo
            .set_home_chain_if_unset(&addr, req.chain_id as i64)
            .await
            .map_err(domain_err_to_status)?;

        Ok(Response::new(SetUserHomeChainResponse {}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{PgCredentialRepository, PgUserRepository};
    use crate::grpc::{user_service_client::UserServiceClient, UserServiceServer};
    use sqlx::PgPool;
    use tokio_stream::wrappers::TcpListenerStream;
    use tonic::transport::Server;

    async fn start_test_server(pool: PgPool) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let svc = UserServiceServer::new(UserServiceImpl {
            user_repo: Arc::new(PgUserRepository::new(pool.clone())),
            credential_repo: Arc::new(PgCredentialRepository::new(pool)),
        });
        tokio::spawn(async move {
            Server::builder()
                .add_service(svc)
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
                .unwrap();
        });
        format!("http://{addr}")
    }

    async fn client(addr: &str) -> UserServiceClient<tonic::transport::Channel> {
        UserServiceClient::connect(addr.to_string()).await.unwrap()
    }

    #[sqlx::test]
    async fn test_create_user_success(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let resp = client(&addr)
            .await
            .create_user(CreateUserRequest {
                display_name: Some("Bob".to_string()),
                credential_id: b"cred-bytes".to_vec(),
                public_key: b"pk-bytes".to_vec(),
                tempo_address: "0xaaaa111111111111111111111111111111111111".to_string(),
            })
            .await
            .unwrap()
            .into_inner();
        assert!(!resp.user_id.is_empty());
        Uuid::parse_str(&resp.user_id).expect("user_id must be a valid UUID");
    }

    #[sqlx::test]
    async fn test_create_user_invalid_address(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let err = client(&addr)
            .await
            .create_user(CreateUserRequest {
                display_name: None,
                credential_id: b"cred".to_vec(),
                public_key: b"pk".to_vec(),
                tempo_address: "not-an-address".to_string(),
            })
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[sqlx::test]
    async fn test_create_user_duplicate_address(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let tempo_addr = "0xaaaa111111111111111111111111111111111111";
        client(&addr)
            .await
            .create_user(CreateUserRequest {
                display_name: None,
                credential_id: b"cred1".to_vec(),
                public_key: b"pk1".to_vec(),
                tempo_address: tempo_addr.to_string(),
            })
            .await
            .unwrap();
        let err = client(&addr)
            .await
            .create_user(CreateUserRequest {
                display_name: None,
                credential_id: b"cred2".to_vec(),
                public_key: b"pk2".to_vec(),
                tempo_address: tempo_addr.to_string(),
            })
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::AlreadyExists);
    }

    #[sqlx::test]
    async fn test_get_user_by_address_success(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let tempo_addr = "0xbbbb222222222222222222222222222222222222";
        client(&addr)
            .await
            .create_user(CreateUserRequest {
                display_name: Some("Charlie".to_string()),
                credential_id: b"cred".to_vec(),
                public_key: b"pk".to_vec(),
                tempo_address: tempo_addr.to_string(),
            })
            .await
            .unwrap();
        let resp = client(&addr)
            .await
            .get_user_by_address(GetUserByAddressRequest {
                tempo_address: tempo_addr.to_string(),
            })
            .await
            .unwrap()
            .into_inner();
        assert_eq!(resp.display_name, "Charlie");
        assert_eq!(resp.status, "active");
        assert_eq!(resp.tempo_address, tempo_addr);
        assert!(!resp.user_id.is_empty());
        Uuid::parse_str(&resp.user_id).expect("user_id must be a valid UUID");
    }

    #[sqlx::test]
    async fn test_get_user_by_address_not_found(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let err = client(&addr)
            .await
            .get_user_by_address(GetUserByAddressRequest {
                tempo_address: "0xdddd444444444444444444444444444444444444".to_string(),
            })
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::NotFound);
    }

    #[sqlx::test]
    async fn test_revoke_last_credential_fails(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let tempo_addr = "0xeeee555555555555555555555555555555555555";
        let create_resp = client(&addr)
            .await
            .create_user(CreateUserRequest {
                display_name: None,
                credential_id: b"only-cred".to_vec(),
                public_key: b"pk".to_vec(),
                tempo_address: tempo_addr.to_string(),
            })
            .await
            .unwrap()
            .into_inner();
        let err = client(&addr)
            .await
            .revoke_credential(RevokeCredentialRequest {
                user_id: create_resp.user_id,
                credential_id: b"only-cred".to_vec(),
            })
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::FailedPrecondition);
    }

    #[sqlx::test]
    async fn test_update_user_display_name(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let create_resp = client(&addr)
            .await
            .create_user(CreateUserRequest {
                display_name: Some("Initial".to_string()),
                credential_id: b"cred".to_vec(),
                public_key: b"pk".to_vec(),
                tempo_address: "0xffff666666666666666666666666666666666666".to_string(),
            })
            .await
            .unwrap()
            .into_inner();
        let update_resp = client(&addr)
            .await
            .update_user(UpdateUserRequest {
                user_id: create_resp.user_id,
                display_name: Some("Updated".to_string()),
            })
            .await
            .unwrap()
            .into_inner();
        assert_eq!(update_resp.display_name, "Updated");
    }

    #[sqlx::test]
    async fn test_set_username_success(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let create_resp = client(&addr)
            .await
            .create_user(CreateUserRequest {
                display_name: Some("Bob".to_string()),
                credential_id: b"cred-bob".to_vec(),
                public_key: b"pk-bob".to_vec(),
                tempo_address: "0xaaaa111111111111111111111111111111111111".to_string(),
            })
            .await
            .unwrap()
            .into_inner();

        let set_resp = client(&addr)
            .await
            .set_username(SetUsernameRequest {
                user_id: create_resp.user_id.clone(),
                username: "bob_test".to_string(),
            })
            .await
            .unwrap()
            .into_inner();

        assert_eq!(set_resp.username, "bob_test");
        assert_eq!(set_resp.user_id, create_resp.user_id);
    }

    #[sqlx::test]
    async fn test_set_username_invalid_format(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let create_resp = client(&addr)
            .await
            .create_user(CreateUserRequest {
                display_name: None,
                credential_id: b"cred-x".to_vec(),
                public_key: b"pk-x".to_vec(),
                tempo_address: "0xbbbb222222222222222222222222222222222222".to_string(),
            })
            .await
            .unwrap()
            .into_inner();

        let err = client(&addr)
            .await
            .set_username(SetUsernameRequest {
                user_id: create_resp.user_id,
                username: "1invalid".to_string(), // starts with digit
            })
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[sqlx::test]
    async fn test_set_username_taken(pool: PgPool) {
        let addr = start_test_server(pool).await;

        // User 1 claims the username.
        let resp1 = client(&addr)
            .await
            .create_user(CreateUserRequest {
                display_name: None,
                credential_id: b"cred-u1".to_vec(),
                public_key: b"pk-u1".to_vec(),
                tempo_address: "0xcccc333333333333333333333333333333333333".to_string(),
            })
            .await
            .unwrap()
            .into_inner();
        client(&addr)
            .await
            .set_username(SetUsernameRequest {
                user_id: resp1.user_id,
                username: "taken_name".to_string(),
            })
            .await
            .unwrap();

        // User 2 tries the same username (different case).
        let resp2 = client(&addr)
            .await
            .create_user(CreateUserRequest {
                display_name: None,
                credential_id: b"cred-u2".to_vec(),
                public_key: b"pk-u2".to_vec(),
                tempo_address: "0xdddd444444444444444444444444444444444444".to_string(),
            })
            .await
            .unwrap()
            .into_inner();
        let err = client(&addr)
            .await
            .set_username(SetUsernameRequest {
                user_id: resp2.user_id,
                username: "Taken_Name".to_string(),
            })
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::AlreadyExists);
    }

    #[sqlx::test]
    async fn test_get_user_by_username_success(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let create_resp = client(&addr)
            .await
            .create_user(CreateUserRequest {
                display_name: Some("Charlie".to_string()),
                credential_id: b"cred-charlie".to_vec(),
                public_key: b"pk-charlie".to_vec(),
                tempo_address: "0xeeee555555555555555555555555555555555555".to_string(),
            })
            .await
            .unwrap()
            .into_inner();
        client(&addr)
            .await
            .set_username(SetUsernameRequest {
                user_id: create_resp.user_id.clone(),
                username: "charlie99".to_string(),
            })
            .await
            .unwrap();

        // Look up by different casing.
        let lookup = client(&addr)
            .await
            .get_user_by_username(GetUserByUsernameRequest {
                username: "Charlie99".to_string(),
            })
            .await
            .unwrap()
            .into_inner();

        assert_eq!(lookup.user_id, create_resp.user_id);
        assert_eq!(lookup.display_name, "Charlie");
        assert_eq!(lookup.username, "charlie99");
        assert_eq!(
            lookup.tempo_address,
            "0xeeee555555555555555555555555555555555555"
        );
    }

    #[sqlx::test]
    async fn test_get_user_by_username_not_found(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let err = client(&addr)
            .await
            .get_user_by_username(GetUserByUsernameRequest {
                username: "nobody123".to_string(),
            })
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::NotFound);
    }

    #[sqlx::test]
    async fn test_home_chain_first_set_then_sticky(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let tempo = "0x1111111111111111111111111111111111111111";
        client(&addr)
            .await
            .create_user(CreateUserRequest {
                display_name: Some("Homer".to_string()),
                credential_id: b"cred-home".to_vec(),
                public_key: b"pk-home".to_vec(),
                tempo_address: tempo.to_string(),
            })
            .await
            .unwrap();

        let unset = client(&addr)
            .await
            .get_user_home_chain(GetUserHomeChainRequest {
                tempo_address: tempo.to_string(),
            })
            .await
            .unwrap()
            .into_inner();
        assert!(!unset.found);

        client(&addr)
            .await
            .set_user_home_chain(SetUserHomeChainRequest {
                tempo_address: tempo.to_string(),
                chain_id: 1337,
            })
            .await
            .unwrap();

        let first = client(&addr)
            .await
            .get_user_home_chain(GetUserHomeChainRequest {
                tempo_address: tempo.to_string(),
            })
            .await
            .unwrap()
            .into_inner();
        assert!(first.found);
        assert_eq!(first.chain_id, 1337);

        client(&addr)
            .await
            .set_user_home_chain(SetUserHomeChainRequest {
                tempo_address: tempo.to_string(),
                chain_id: 9999,
            })
            .await
            .unwrap();

        let still = client(&addr)
            .await
            .get_user_home_chain(GetUserHomeChainRequest {
                tempo_address: tempo.to_string(),
            })
            .await
            .unwrap()
            .into_inner();
        assert_eq!(still.chain_id, 1337);
    }

    #[sqlx::test]
    async fn test_get_user_home_chain_unknown_address(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let res = client(&addr)
            .await
            .get_user_home_chain(GetUserHomeChainRequest {
                tempo_address: "0x2222222222222222222222222222222222222222".to_string(),
            })
            .await
            .unwrap()
            .into_inner();
        assert!(!res.found);
    }
}
