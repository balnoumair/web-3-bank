pub mod user_service;

tonic::include_proto!("user.v1");

pub use user_service_server::UserServiceServer;
