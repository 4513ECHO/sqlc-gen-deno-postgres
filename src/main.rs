use case::CaseExt;
use enquote::unquote;
use indoc::formatdoc;
use prost::Message;
use std::collections::HashMap;
use std::io;
use std::io::{Cursor, Read, Write};
use std::process::abort;

pub mod plugin {
    include!(concat!(env!("OUT_DIR"), "/plugin.rs"));
}

macro_rules! concat_string {
    ($base_str:expr, $($arg:expr),+) => {
        $($base_str.push_str(&$arg.to_owned());)*
    };
}

fn deserialize_codegen_request(buf: &[u8]) -> plugin::GenerateRequest {
    plugin::GenerateRequest::decode(&mut Cursor::new(buf)).unwrap_or_else(|_| abort())
}

fn serialize_codegen_response(resp: &plugin::GenerateResponse) -> Vec<u8> {
    let mut buf = Vec::with_capacity(resp.encoded_len());

    resp.encode(&mut buf).unwrap_or_else(|_| abort());
    buf
}

fn create_querier(query: plugin::Query) -> String {
    let mut querier = String::new();

    concat_string!(
        querier,
        "const ",
        query.name.to_camel_lowercase(),
        "Query = `",
        query.text,
        "`;\n\n"
    );

    if !query.params.is_empty() {
        concat_string!(
            querier,
            "export type ",
            query.name,
            "Params = {\n",
            query
                .params
                .iter()
                .flat_map(|param| &param.column)
                .map(|column| format!(
                    "  {}: {};",
                    column.name.to_camel_lowercase(),
                    to_ts_type(column)
                ))
                .collect::<Vec<_>>()
                .join("\n")
                .as_str(),
            "\n};\n\n"
        );
    }

    if query.cmd != ":exec" {
        concat_string!(
            querier,
            "export type ",
            query.name,
            "Row = {\n",
            query
                .columns
                .iter()
                .map(|column| format!(
                    "  {}: {};",
                    column.name.to_camel_lowercase(),
                    to_ts_type(column)
                ))
                .collect::<Vec<_>>()
                .join("\n")
                .as_str(),
            "\n};\n\n"
        );
    }

    concat_string!(
        querier,
        "export async function ",
        query.name.to_camel_lowercase(),
        "(\n  client: Client"
    );
    if !query.params.is_empty() {
        concat_string!(querier, ",\n  params: ", query.name, "Params");
    }
    // TODO: Use promise chain
    concat_string!(
        querier,
        ",\n): Promise<",
        match query.cmd.as_str() {
            ":exec" => "void".to_string(),
            ":one" => query.name.clone() + "Row | null",
            ":many" => query.name.clone() + "Row[]",
            _ => abort(),
        },
        "> {\n  const { rows } = await client.queryObject<",
        match query.cmd.as_str() {
            ":exec" => "unknown".to_string(),
            ":one" | ":many" => query.name.clone() + "Row",
            _ => abort(),
        },
        ">({\n"
    );
    if !query.params.is_empty() {
        concat_string!(querier, "    args: [", build_params(&query.params), "],\n");
    }
    concat_string!(
        querier,
        "    camelcase: true,\n    text: ",
        query.name.to_camel_lowercase(),
        "Query,\n  });\n  return ",
        match query.cmd.as_str() {
            ":exec" => "",
            ":one" => "rows[0] ?? null",
            ":many" => "rows",
            _ => abort(),
        },
        ";\n}"
    );

    querier
}

fn build_params(params: &[plugin::Parameter]) -> String {
    params
        .iter()
        .map(|param| {
            let column = param.column.as_ref().unwrap_or_else(|| abort());
            "params.".to_string()
                + column.name.to_camel_lowercase().as_str()
                + if column.is_sqlc_slice { "[0]" } else { "" }
        })
        .collect::<Vec<_>>()
        .join(", ")
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

fn parse_plugin_options(options: &[u8]) -> HashMap<String, String> {
    let binding = String::from_utf8(options.to_vec()).unwrap_or_else(|_| abort());
    unquote(&binding)
        .unwrap_or_else(|_| abort())
        .split(',')
        .map(|s| {
            let [key, value] = s.split('=').collect::<Vec<_>>()[..] else {
                abort()
            };
            (key.to_string(), unquote(value).unwrap_or_else(|_| abort()))
        })
        .collect::<HashMap<_, _>>()
}

fn create_codegen_response(req: plugin::GenerateRequest) -> plugin::GenerateResponse {
    let options = parse_plugin_options(&req.plugin_options);

    let mut contents = formatdoc!(
        "
        // Generated by sqlc-gen-deno-postgres. DO NOT EDIT.
        // versions:
        //   sqlc {}
        //   sqlc-gen-deno-postgres v{}
        ",
        req.sqlc_version,
        env!("CARGO_PKG_VERSION"),
    );

    concat_string!(
        contents,
        "import { Client } from \"",
        match options.get("import_url") {
            Some(import_url) => import_url,
            None => "https://deno.land/x/postgres@v0.17.0/mod.ts",
        },
        "\";\n\n",
        req.queries
            .into_iter()
            .map(create_querier)
            .collect::<Vec<_>>()
            .join("\n\n")
    );

    let file = plugin::File {
        name: "querier.ts".to_string(),
        contents: contents.as_bytes().to_vec(),
    };

    plugin::GenerateResponse { files: vec![file] }
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
