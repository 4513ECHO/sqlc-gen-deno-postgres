use prost::Message;
use std::io;
use std::io::{Cursor, Read, Write};
use std::process::abort;

pub mod plugin {
    include!(concat!(env!("OUT_DIR"), "/plugin.rs"));
}

fn deserialize_codegen_request(buf: &[u8]) -> plugin::CodeGenRequest {
    plugin::CodeGenRequest::decode(&mut Cursor::new(buf)).unwrap_or_else(|_| abort())
}

fn serialize_codegen_response(resp: &plugin::CodeGenResponse) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.reserve(resp.encoded_len());

    resp.encode(&mut buf).unwrap_or_else(|_| abort());
    buf
}

fn create_codegen_response(req: plugin::CodeGenRequest) -> plugin::CodeGenResponse {
    let file = plugin::File {
        name: "hello.txt".to_string(),
        contents: ("Hello World from ".to_owned() + &req.sqlc_version)
            .as_bytes()
            .to_vec(),
    };

    let mut resp = plugin::CodeGenResponse::default();
    resp.files.push(file);
    resp
}

fn main() {
    let mut stdin = Vec::new();
    io::stdin()
        .read_to_end(&mut stdin)
        .unwrap_or_else(|_| abort());

    let request = deserialize_codegen_request(&stdin);
    let resp = create_codegen_response(request);
    let out = serialize_codegen_response(&resp);

    io::stdout().write_all(&out).unwrap_or_else(|_| abort());
}
