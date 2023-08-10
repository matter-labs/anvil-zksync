# Build the system contracts
build-contracts:
	cd etc/system-contracts && yarn; yarn install; yarn build; yarn preprocess; yarn build-bootloader
	./scripts/refresh_contracts.sh

# Clean the system contracts
clean-contracts:
	cd etc/system-contracts && yarn clean
	rm -rf src/deps/contracts

# Rebuild the system contracts
rebuild-contracts:
	cd etc/system-contracts && yarn build; yarn preprocess; yarn build-bootloader
	./scripts/refresh_contracts.sh

# Build the Rust project
rust-build:
	cargo build --release

# Build the Rust documentation
rust-doc:
	cargo doc --no-deps --open

# Lint checks for Rust code
lint:
	cargo fmt --all -- --check
	cargo clippy -Zunstable-options -- -D warnings --allow clippy::unwrap_used

# Run unit tests for Rust code
test:
	cargo test

# Run e2e tests against running era_test_node
test-e2e:
	./scripts/execute-e2e-tests.sh

# Build everything
all: build-contracts rust-build

# Clean everything
clean: clean-contracts

.PHONY: build-contracts clean-contracts rebuild-contracts rust-build lint test test-e2e all clean
