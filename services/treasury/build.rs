fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(
            &["../../packages/proto/treasury/treasury_service.proto"],
            &["../../packages/proto"],
        )?;
    Ok(())
}
