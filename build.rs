extern crate prost_build;

fn main() {
    let mut prost_build = prost_build::Config::new();
    // NOTE: This flag is required because the feature is enabled by default since protoc v3.15.0,
    // but protoc in the `ubuntu-latest` image of GitHub Actions is v3.12.4.
    // References:
    // https://github.com/protocolbuffers/protobuf/releases/tag/v3.15.0
    // https://packages.ubuntu.com/jammy/protobuf-compiler
    prost_build.protoc_arg("--experimental_allow_proto3_optional");
    prost_build
        .compile_protos(&["src/codegen.proto"], &["src/"])
        .unwrap();
}
