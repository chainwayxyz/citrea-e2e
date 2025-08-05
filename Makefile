.PHONY: help

help: ## Display this help message
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

.PHONY: build
build: ## Build the project
	@cargo build


clean: ## Cleans compiled
	@cargo clean

install-dev-tools:  ## Installs all necessary cargo helpers
	cargo install --locked dprint
	cargo install cargo-llvm-cov
	cargo install cargo-hack
	cargo install --locked cargo-udeps
	cargo install flaky-finder
	cargo install --locked cargo-nextest
	cargo install --version 1.7.0 cargo-binstall
	cargo binstall --no-confirm cargo-risczero@1.0.5
	cargo risczero install --version r0.1.79.0-2
	rustup target add thumbv6m-none-eabi
	rustup component add llvm-tools-preview

lint:  ## cargo check and clippy. Skip clippy on guest code since it's not supported by risc0
	## fmt first, because it's the cheapest
	dprint check
	cargo +nightly fmt --all --check
	cargo check --all-targets --all-features
	cargo clippy --all-targets --all-features

lint-fix:  ## dprint fmt, cargo fmt, fix and clippy. Skip clippy on guest code since it's not supported by risc0
	dprint fmt
	cargo +nightly fmt --all
	cargo fix --allow-dirty --all-features
	cargo clippy --fix --allow-dirty --all-features

docs:  ## Generates documentation locally
	cargo doc --open

set-git-hook:
	git config core.hooksPath .githooks

test-nocapture: ## Runs test suite with output from tests printed
	RISC0_DEV_MODE=1 PARALLEL_PROOF_LIMIT=1 cargo nextest run --no-capture --retries 0 --workspace --all-features --no-fail-fast $(filter-out $@,$(MAKECMDGOALS))

test: ## Runs test suite using nextest
	RISC0_DEV_MODE=1 PARALLEL_PROOF_LIMIT=1 cargo nextest run -j15 --locked --workspace --all-features --no-fail-fast $(filter-out $@,$(MAKECMDGOALS))
