fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile(
            &["../../packages/proto/user/v1/user_service.proto"],
            &["../../packages/proto"],
        )?;
    Ok(())
}
