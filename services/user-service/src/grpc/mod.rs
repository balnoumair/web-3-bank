pub mod user_service;

tonic::include_proto!("user");

pub use user_service_server::UserServiceServer;
