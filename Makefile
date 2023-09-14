ROOT_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
WASM_FILE := target/wasm32-wasi/$(if $(RELEASE),release,debug)/sqlc-gen-deno-postgres.wasm
VERSION := $(shell git describe --tags --abbrev=0)
RELEASE_URL := https://github.com/4513ECHO/sqlc-gen-deno-postgres/releases/download/$(VERSION)/sqlc-gen-deno-postgres.wasm

.PHONY: build
build: build.rs src/codegen.proto src/main.rs
ifeq ($(RELEASE),)
	cargo +nightly build --target wasm32-wasi
else
	RUSTFLAGS="-Zlocation-detail=none" cargo +nightly build --release --target wasm32-wasi
endif
	@du -h $(WASM_FILE)

$(WASM_FILE): build

src/codegen.proto:
	curl -o $@ -L https://github.com/sqlc-dev/sqlc/raw/v1.21.0/protos/plugin/codegen.proto

sqlc.json: $(WASM_FILE) _sqlc.json
	SHA256=$(shell sha256sum $< | awk '{print $$1}') \
		URL=$(if $(RELEASE),"$(RELEASE_URL)","file://$(ROOT_DIR)$<") \
		envsubst > $@ < _sqlc.json
