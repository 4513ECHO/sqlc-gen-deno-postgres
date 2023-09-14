use cruet::Inflector;
use prost::Message;
use std::io;
use std::io::{Cursor, Read, Write};
use std::process::abort;

pub mod plugin {
    include!(concat!(env!("OUT_DIR"), "/plugin.rs"));
}

macro_rules! fprintf {
    ($base_str:expr, $format:literal, $($arg:tt)*) => {
        $base_str.push_str(format!($format, $($arg)*).as_str());
    };
}

fn deserialize_codegen_request(buf: &[u8]) -> plugin::CodeGenRequest {
    plugin::CodeGenRequest::decode(&mut Cursor::new(buf)).unwrap_or_else(|_| abort())
}

fn serialize_codegen_response(resp: &plugin::CodeGenResponse) -> Vec<u8> {
    let mut buf = Vec::with_capacity(resp.encoded_len());

    resp.encode(&mut buf).unwrap_or_else(|_| abort());
    buf
}

const IMPORT_STATEMENT: &str =
    r#"import { Client } from "https://deno.land/x/postgres@v0.17.0/mod.ts";"#;

fn create_querier(query: plugin::Query) -> String {
    let mut querier = String::new();

    fprintf!(
        querier,
        "const {}Query = `{}`;\n\n",
        query.name.to_camel_case(),
        query.text
    );

    if !query.params.is_empty() {
        fprintf!(
            querier,
            "export type {}Params = {{\n{}\n}};\n\n",
            query.name,
            query
                .params
                .iter()
                .flat_map(|param| &param.column)
                .map(|column| format!("  {}: {};", column.name.to_camel_case(), to_ts_type(column)))
                .collect::<Vec<_>>()
                .join("\n")
                .as_str(),
        );
    }

    fprintf!(
        querier,
        "export async function {}(\n  client: Client",
        query.name.to_camel_case()
    );
    if !query.params.is_empty() {
        fprintf!(querier, ",\n  params: {}Params", query.name);
    }
    fprintf!(
        querier,
        ",\n): Promise<{}> {{\n  // TODO\n}};\n",
        match query.cmd.as_str() {
            ":exec" => "void".to_string(),
            ":one" => query.name.clone() + "Row | null",
            ":many" => query.name.clone() + "Row[]",
            _ => abort(),
        }
    );

    querier
}

fn to_ts_type(column: &plugin::Column) -> String {
    let binding = column
        .r#type
        .as_ref()
        .unwrap_or_else(|| abort())
        .name
        .to_lowercase();
    let type_name = binding.as_str();
    // References:
    // https://github.com/denodrivers/postgres/blob/v0.17.0/query/decode.ts
    // https://github.com/sqlc-dev/sqlc/blob/v1.21.0/internal/codegen/golang/postgresql_type.go
    match type_name {
        "serial" | "serial4" | "pg_catalog.serial4" => "number",
        "bigserial" | "serial8" | "pg_catalog.serial8" => "bigint",
        "smallserial" | "serial2" | "pg_catalog.serial2" => "number",
        "integer" | "int" | "int4" | "pg_catalog.int4" => "number",
        "bigint" | "int8" | "pg_catalog.int8" => "bigint",
        "smallint" | "int2" | "pg_catalog.int2" => "number",
        "float" | "double precision" | "float8" | "pg_catalog.float8" => "number",
        "real" | "float4" | "pg_catalog.float4" => "number",
        "numeric" | "pg_catalog.numeric" | "money" => "string",
        "boolean" | "bool" | "pg_catalog.bool" => "boolean",
        "json" | "jsonb" => "unknown",
        "bytea" | "blob" | "pg_catalog.bytea" => "Uint8Array",
        "date" => "Date",
        "pg_catalog.time" | "pg_catalog.timetz" => "string",
        "pg_catalog.timestamp" | "pg_catalog.timestamptz" | "timestamptz" => "Date",
        "text" | "pg_catalog.varchar" | "pg_catalog.bpchar" => "string",
        "string" | "citext" | "name" => "string",
        "uuid" => "string",
        "any" => "unknown",
        // TODO: Support other types (range, id, etc.)
        _ => type_name,
    }
    .to_string()
        + if column.is_sqlc_slice { "[]" } else { "" }
        + if column.not_null { "" } else { " | null" }
}

fn create_codegen_response(req: plugin::CodeGenRequest) -> plugin::CodeGenResponse {
    let contents = IMPORT_STATEMENT.to_string()
        + "\n\n"
        + req
            .queries
            .into_iter()
            .map(create_querier)
            .collect::<Vec<_>>()
            .join("\n\n")
            .as_str();
    let contents = contents.as_bytes().to_vec();

    let file = plugin::File {
        name: "querier.ts".to_string(),
        contents,
    };

    plugin::CodeGenResponse { files: vec![file] }
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
