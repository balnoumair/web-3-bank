// services/treasury/build.rs
fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path()
        .expect("protoc-bin-vendored: could not find vendored protoc binary");
    std::env::set_var("PROTOC", &protoc);
    tonic_build::compile_protos("proto/treasury.proto")
        .expect("failed to compile proto/treasury.proto");
}
