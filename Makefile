# Build the system contracts
build-contracts:
	cd etc/system-contracts && yarn; yarn install; yarn build
	./scripts/refresh_contracts.sh

# Clean the system contracts
clean-contracts:
	cd etc/system-contracts && yarn clean
	rm -rf src/deps/contracts

# Rebuild the system contracts
rebuild-contracts:
	cd etc/system-contracts && yarn build
	./scripts/refresh_contracts.sh
	./scripts/refresh_test_contracts.sh

# Build the Rust project
rust-build:
	cargo build --release

# Run local
run: rust-build
	./target/release/era_test_node run

# Build the Rust project for a specific target. Primarily used for CI.
build-%:
	cross build --bin era_test_node --target $* --release

# Build the Rust documentation
rust-doc:
	cargo doc --no-deps --open

# Lint checks for Rust code
lint:
	cd e2e-tests && yarn && yarn lint && yarn fmt && yarn typecheck
	cargo fmt --all -- --check
	cargo clippy -p era_test_node -Zunstable-options -- -D warnings --allow clippy::unwrap_used

# Fix lint errors for Rust code
lint-fix:
	cd e2e-tests && yarn && yarn lint:fix && yarn fmt:fix
	cargo clippy --fix
	cargo fmt

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

# Create new draft release based on Cargo.toml version
new-release-tag:
	@VERSION_NUMBER=$$(grep '^version =' Cargo.toml | awk -F '"' '{print $$2}') && \
	git tag -a v$$VERSION_NUMBER -m "Release v$$VERSION_NUMBER" && \
	echo "\n\033[0;32mGit tag creation SUCCESSFUL! Use the following command to push the tag:\033[0m" && \
	echo "git push origin v$$VERSION_NUMBER"

# Create the rust book
book:
	mdbook build docs/rustbook

.PHONY: build-contracts clean-contracts rebuild-contracts rust-build lint test test-e2e all clean build-% new-release-tag book
