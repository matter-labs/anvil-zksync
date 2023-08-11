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

# Build the Rust project for a specific target. Primarily used for CI.
build-%:
	cross build --bin era_test_node --target $* --release

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

# Build everything
all: build-contracts rust-build

# Clean everything
clean: clean-contracts

.PHONY: build-contracts clean-contracts rebuild-contracts rust-build lint test all clean
