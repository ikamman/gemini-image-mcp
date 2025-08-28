# Makefile for Gemini Image MCP Server
.DEFAULT_GOAL := help
.PHONY: help build test lint format check clean install dev release ci pre-commit

# Colors for output
RED := \033[0;31m
GREEN := \033[0;32m
YELLOW := \033[1;33m
BLUE := \033[0;34m
NC := \033[0m # No Color

# Project info
PROJECT_NAME := gemini-image-mcp
RUST_VERSION := 1.70.0

## Display this help message
help:
	@echo "$(GREEN)$(PROJECT_NAME) - Makefile Commands$(NC)"
	@echo ""
	@echo "$(YELLOW)Development:$(NC)"
	@echo "  build          - Build the project in debug mode"
	@echo "  build-release  - Build the project in release mode"
	@echo "  test           - Run all tests"
	@echo "  dev            - Run the server in development mode"
	@echo "  clean          - Clean build artifacts"
	@echo ""
	@echo "$(YELLOW)Code Quality:$(NC)"
	@echo "  lint           - Run clippy linter"
	@echo "  format         - Format code with rustfmt"
	@echo "  format-check   - Check if code is properly formatted"
	@echo "  check          - Run cargo check"
	@echo ""
	@echo "$(YELLOW)CI/CD:$(NC)"
	@echo "  ci             - Run full CI pipeline (format, lint, test, build)"
	@echo "  pre-commit     - Run pre-commit checks (format, lint)"
	@echo ""
	@echo "$(YELLOW)Installation:$(NC)"
	@echo "  install        - Install the binary"
	@echo "  install-deps   - Install development dependencies"

## Build the project in debug mode
build:
	@echo "$(BLUE)Building $(PROJECT_NAME) in debug mode...$(NC)"
	cargo build

## Build the project in release mode
build-release:
	@echo "$(BLUE)Building $(PROJECT_NAME) in release mode...$(NC)"
	cargo build --release

## Run all tests
test:
	@echo "$(BLUE)Running tests...$(NC)"
	cargo test

## Run tests with output
test-verbose:
	@echo "$(BLUE)Running tests with verbose output...$(NC)"
	cargo test -- --nocapture

## Run clippy linter
lint:
	@echo "$(BLUE)Running clippy linter...$(NC)"
	cargo clippy -- -D warnings

## Format code with rustfmt
format:
	@echo "$(BLUE)Formatting code...$(NC)"
	cargo fmt

## Check if code is properly formatted
format-check:
	@echo "$(BLUE)Checking code formatting...$(NC)"
	cargo fmt --check

## Run cargo check
check:
	@echo "$(BLUE)Running cargo check...$(NC)"
	cargo check

## Clean build artifacts
clean:
	@echo "$(BLUE)Cleaning build artifacts...$(NC)"
	cargo clean

## Install the binary
install:
	@echo "$(BLUE)Installing $(PROJECT_NAME)...$(NC)"
	cargo install --path .

## Install development dependencies
install-deps:
	@echo "$(BLUE)Installing development dependencies...$(NC)"
	@command -v cargo-audit >/dev/null 2>&1 || cargo install cargo-audit
	@command -v cargo-tarpaulin >/dev/null 2>&1 || cargo install cargo-tarpaulin
	@echo "$(GREEN)Development dependencies installed$(NC)"

## Run the server in development mode
dev:
	@echo "$(BLUE)Running $(PROJECT_NAME) in development mode...$(NC)"
	cargo run

## Run the server with example API key
dev-with-key:
	@echo "$(BLUE)Running $(PROJECT_NAME) with example API key...$(NC)"
	@echo "$(YELLOW)Note: Replace 'your-api-key' with actual Gemini API key$(NC)"
	cargo run -- --gemini-api-key "your-api-key"

## Run pre-commit checks
pre-commit: format-check lint
	@echo "$(GREEN)Pre-commit checks passed!$(NC)"

## Run full CI pipeline
ci: format-check lint test build build-release
	@echo "$(GREEN)CI pipeline completed successfully!$(NC)"

## Security audit
audit:
	@echo "$(BLUE)Running security audit...$(NC)"
	cargo audit

## Generate test coverage report
coverage:
	@echo "$(BLUE)Generating test coverage report...$(NC)"
	cargo tarpaulin --out Html --output-dir coverage

## Run benchmarks (if any exist)
bench:
	@echo "$(BLUE)Running benchmarks...$(NC)"
	cargo bench

## Update dependencies
update:
	@echo "$(BLUE)Updating dependencies...$(NC)"
	cargo update

## Check for outdated dependencies
outdated:
	@echo "$(BLUE)Checking for outdated dependencies...$(NC)"
	cargo outdated

## Generate documentation
docs:
	@echo "$(BLUE)Generating documentation...$(NC)"
	cargo doc --no-deps --open

## Run manual tests with example commands
manual-test:
	@echo "$(BLUE)Running manual tests...$(NC)"
	@echo "Testing initialize method..."
	@echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | cargo run --quiet
	@echo ""
	@echo "Testing tools list..."
	@echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | cargo run --quiet

## Docker build (if Dockerfile exists)
docker-build:
	@if [ -f Dockerfile ]; then \
		echo "$(BLUE)Building Docker image...$(NC)"; \
		docker build -t $(PROJECT_NAME) .; \
	else \
		echo "$(YELLOW)No Dockerfile found$(NC)"; \
	fi

## Release preparation
release-prep: ci audit
	@echo "$(GREEN)Release preparation completed!$(NC)"
	@echo "$(YELLOW)Don't forget to:$(NC)"
	@echo "  - Update version in Cargo.toml"
	@echo "  - Create and push git tag (cargo-dist will handle the rest)"

## Show project info
info:
	@echo "$(GREEN)Project Information:$(NC)"
	@echo "Name: $(PROJECT_NAME)"
	@echo "Rust Version: $(shell rustc --version)"
	@echo "Cargo Version: $(shell cargo --version)"
	@echo "Target: $(shell rustc -vV | grep host | cut -d' ' -f2)"