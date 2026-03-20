use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sqlx::PgPool;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::db::{credentials, users};
use crate::grpc::{
    user_service_server::UserService, AddCredentialRequest, AddCredentialResponse,
    CreateUserRequest, CreateUserResponse, Credential, GetUserByAddressRequest,
    GetUserByAddressResponse, ListCredentialsRequest, ListCredentialsResponse,
    RevokeCredentialRequest, RevokeCredentialResponse, UpdateUserRequest, UpdateUserResponse,
};

/// Regex for a 0x-prefixed 40-character hex Ethereum address.
static TEMPO_ADDR_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

fn valid_tempo_address(addr: &str) -> bool {
    TEMPO_ADDR_RE
        .get_or_init(|| regex::Regex::new(r"^0x[0-9a-fA-F]{40}$").unwrap())
        .is_match(addr)
}

fn pg_is_unique_violation(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .and_then(|d| d.code())
        .map(|c| c == "23505")
        .unwrap_or(false)
}

pub struct UserServiceImpl {
    pub pool: PgPool,
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn create_user(
        &self,
        request: Request<CreateUserRequest>,
    ) -> Result<Response<CreateUserResponse>, Status> {
        let req = request.into_inner();

        if !valid_tempo_address(&req.tempo_address) {
            return Err(Status::invalid_argument(
                "tempo_address must be a 0x-prefixed 40-char hex string",
            ));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let name = req.display_name.as_deref().unwrap_or("");
        let user_row = sqlx::query!(
            "INSERT INTO users.users (display_name) VALUES ($1) RETURNING id",
            name
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

        let user_id = user_row.id;

        let cred_result = sqlx::query!(
            "INSERT INTO users.credentials (user_id, credential_id, public_key, tempo_address) VALUES ($1, $2, $3, $4) RETURNING id",
            user_id,
            req.credential_id.as_slice(),
            req.public_key.as_slice(),
            req.tempo_address,
        )
        .fetch_one(&mut *tx)
        .await;

        match cred_result {
            Ok(_) => {}
            Err(e) => {
                drop(tx); // auto-rollback
                if pg_is_unique_violation(&e) {
                    return Err(Status::already_exists(
                        "address or credential already registered",
                    ));
                }
                return Err(Status::internal(e.to_string()));
            }
        }

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(CreateUserResponse {
            user_id: user_id.to_string(),
        }))
    }

    async fn get_user_by_address(
        &self,
        request: Request<GetUserByAddressRequest>,
    ) -> Result<Response<GetUserByAddressResponse>, Status> {
        let req = request.into_inner();

        let row = credentials::get_user_by_address(&self.pool, &req.tempo_address)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("user not found for given tempo_address"))?;

        Ok(Response::new(GetUserByAddressResponse {
            user_id: row.user_id.to_string(),
            display_name: row.display_name,
            status: row.status,
            tempo_address: row.tempo_address,
        }))
    }

    async fn list_credentials(
        &self,
        request: Request<ListCredentialsRequest>,
    ) -> Result<Response<ListCredentialsResponse>, Status> {
        let req = request.into_inner();

        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|_| Status::invalid_argument("user_id must be a valid UUID"))?;

        let rows = credentials::list_credentials(&self.pool, user_id, false)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let creds = rows
            .into_iter()
            .map(|r| Credential {
                credential_id: URL_SAFE_NO_PAD.encode(&r.credential_id),
                tempo_address: r.tempo_address,
                created_at: r.created_at.to_rfc3339(),
                revoked: r.revoked_at.is_some(),
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

        if !valid_tempo_address(&req.tempo_address) {
            return Err(Status::invalid_argument(
                "tempo_address must be a 0x-prefixed 40-char hex string",
            ));
        }

        users::get_user_by_id(&self.pool, user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("user not found"))?;

        credentials::insert_credential(
            &self.pool,
            user_id,
            &req.credential_id,
            &req.public_key,
            &req.tempo_address,
        )
        .await
        .map_err(|e| match &e {
            credentials::CredentialError::Db(db_err) if pg_is_unique_violation(db_err) => {
                Status::already_exists("tempo_address already registered")
            }
            _ => Status::internal(e.to_string()),
        })?;

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

        // Check user exists first
        let row = users::get_user_by_id(&self.pool, user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("user not found"))?;

        // Only update if display_name is provided (proto3 optional)
        if let Some(name) = &req.display_name {
            users::update_display_name(&self.pool, user_id, name)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            // Re-fetch to get updated display_name
            let updated = users::get_user_by_id(&self.pool, user_id)
                .await
                .map_err(|e| Status::internal(e.to_string()))?
                .ok_or_else(|| Status::not_found("user not found"))?;
            return Ok(Response::new(UpdateUserResponse {
                user_id: updated.id.to_string(),
                display_name: updated.display_name,
            }));
        }

        Ok(Response::new(UpdateUserResponse {
            user_id: row.id.to_string(),
            display_name: row.display_name,
        }))
    }

    async fn revoke_credential(
        &self,
        request: Request<RevokeCredentialRequest>,
    ) -> Result<Response<RevokeCredentialResponse>, Status> {
        let req = request.into_inner();

        let user_id = Uuid::parse_str(&req.user_id)
            .map_err(|_| Status::invalid_argument("user_id must be a valid UUID"))?;

        credentials::revoke_credential(&self.pool, user_id, &req.credential_id)
            .await
            .map_err(|e| match e {
                credentials::CredentialError::LastActiveCredential => {
                    Status::failed_precondition("cannot revoke the last active credential")
                }
                credentials::CredentialError::NotFound => {
                    Status::not_found("credential not found or already revoked")
                }
                credentials::CredentialError::Db(db_err) => Status::internal(db_err.to_string()),
            })?;

        Ok(Response::new(RevokeCredentialResponse { success: true }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::{user_service_client::UserServiceClient, UserServiceServer};
    use tokio_stream::wrappers::TcpListenerStream;
    use tonic::transport::Server;

    async fn start_test_server(pool: PgPool) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let svc = UserServiceServer::new(UserServiceImpl { pool });
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

    #[sqlx::test(migrations = "src/db/migrations")]
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

    #[sqlx::test(migrations = "src/db/migrations")]
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

    #[sqlx::test(migrations = "src/db/migrations")]
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

    #[sqlx::test(migrations = "src/db/migrations")]
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

    #[sqlx::test(migrations = "src/db/migrations")]
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

    #[sqlx::test(migrations = "src/db/migrations")]
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

    #[sqlx::test(migrations = "src/db/migrations")]
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
}
