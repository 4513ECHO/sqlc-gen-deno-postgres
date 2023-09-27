ROOT_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
CARGO ?= cargo +nightly
CARGO_EXTRA_FLAGS ?=

filter-false = $(strip $(filter-out 0 off OFF false FALSE False,$1))

RELEASE ?= false
USE_WASM_OPT ?= false
SQLC_VERSION ?= v1.22.0

WASM_FILE := target/wasm32-wasi/$(if $(call filter-false,$(RELEASE)),release,debug)/sqlc-gen-deno-postgres.wasm
VERSION := v$(shell cargo read-manifest | jq -r .version)
RELEASE_URL := $(basename $(shell git remote get-url origin))/releases/download/$(VERSION)/$(notdir $(WASM_FILE))

.DEFAULT_GOAL := $(WASM_FILE)

$(WASM_FILE): build.rs src/codegen.proto src/main.rs
ifeq ($(call filter-false,$(RELEASE)),)
	$(CARGO) build --target wasm32-wasi $(CARGO_EXTRA_FLAGS)
else
	RUSTFLAGS="-Zlocation-detail=none" $(CARGO) build --release --target wasm32-wasi $(CARGO_EXTRA_FLAGS)
ifneq ($(call filter-false,$(USE_WASM_OPT)),)
	wasm-opt -Oz --output $@ $@
endif
endif
	@du -h $@

$(WASM_FILE).sha256: $(WASM_FILE)
	sha256sum $< | awk '{print $$1}' > $@

src/codegen.proto:
	curl -o $@ -L https://github.com/sqlc-dev/sqlc/raw/$(SQLC_VERSION)/protos/plugin/codegen.proto

sqlc.json: $(WASM_FILE).sha256 _sqlc.json
	SHA256=$(shell cat $<) \
	URL=$(if $(GITHUB_ACTIONS),"$(RELEASE_URL)","file://$(ROOT_DIR)$(WASM_FILE)") \
		envsubst > $@ < _sqlc.json
