use tonic::{Request, Response, Status};

pub mod proto {
    tonic::include_proto!("user.v1");
}

use proto::user_service_server::UserService;
use proto::*;

#[derive(Debug, Default)]
pub struct UserServiceImpl {
    pub pool: Option<sqlx::PgPool>,
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn create_user(
        &self,
        _request: Request<CreateUserRequest>,
    ) -> Result<Response<CreateUserResponse>, Status> {
        Err(Status::unimplemented("not implemented"))
    }

    async fn get_user_by_address(
        &self,
        _request: Request<GetUserByAddressRequest>,
    ) -> Result<Response<GetUserByAddressResponse>, Status> {
        Err(Status::unimplemented("not implemented"))
    }

    async fn list_credentials(
        &self,
        _request: Request<ListCredentialsRequest>,
    ) -> Result<Response<ListCredentialsResponse>, Status> {
        Err(Status::unimplemented("not implemented"))
    }

    async fn add_credential(
        &self,
        _request: Request<AddCredentialRequest>,
    ) -> Result<Response<AddCredentialResponse>, Status> {
        Err(Status::unimplemented("not implemented"))
    }

    async fn update_user(
        &self,
        _request: Request<UpdateUserRequest>,
    ) -> Result<Response<UpdateUserResponse>, Status> {
        Err(Status::unimplemented("not implemented"))
    }

    async fn revoke_credential(
        &self,
        _request: Request<RevokeCredentialRequest>,
    ) -> Result<Response<RevokeCredentialResponse>, Status> {
        Err(Status::unimplemented("not implemented"))
    }
}
