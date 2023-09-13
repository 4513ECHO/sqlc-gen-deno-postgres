ROOT_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

.PHONY: build
build: build.rs src/codegen.proto src/main.rs
	cargo build --release --target wasm32-wasi
	@du -h target/wasm32-wasi/release/sqlc-gen-deno-postgres.wasm

src/codegen.proto:
	curl -o $@ -L https://github.com/sqlc-dev/sqlc/raw/v1.21.0/protos/plugin/codegen.proto

sqlc.json: target/wasm32-wasi/release/sqlc-gen-deno-postgres.wasm _sqlc.json
	SHA256=$(shell sha256sum $< | awk '{print $$1}') \
		URL=$(if $(RELEASE),"$(RELEASE_URL)","file://$(ROOT_DIR)$<") \
		envsubst > $@ < _sqlc.json
