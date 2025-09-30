# ==============================================================================
# Rust プロジェクト Makefile
# ==============================================================================

.DEFAULT_GOAL := help

SHELL := /usr/bin/bash
.SHELLFLAGS := -eu -o pipefail -c
.ONESHELL:
.EXPORT_ALL_VARIABLES:

.PHONY: \
  help add-tools clean \
  fmt fmt-check clippy test build release \
  doc coverage \
  audit outdated udeps miri bench \
  check full_local ci

help: ## このヘルプを表示
	@grep -E '^[a-zA-Z0-9_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

# ---- 基本 ---------------------------------------------------------------------
clean: ## target などを削除
	cargo clean

fmt: ## 整形を実行（ローカル用）
	cargo fmt --all

fmt-check: ## 整形済みかを検査（CI向け）
	cargo fmt --all -- --check

clippy: ## リンター（警告=エラー）
	cargo clippy --workspace --all-targets --all-features -- -D warnings

test: ## テスト（警告=エラー）
	RUSTFLAGS="-D warnings" cargo test --workspace --all-features

build: ## デバッグビルド（警告=エラー）
	RUSTFLAGS="-D warnings" cargo build --workspace --all-features

release: ## リリースビルド（警告=エラー）
	RUSTFLAGS="-D warnings" cargo build --workspace --all-features --release

doc: ## ドキュメント生成（警告=エラー）
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features

# ---- ツール導入（未導入時のみ） -----------------------------------------------
add-tools: ## llvm-cov/監査系ツールを未導入なら導入
	rustup component list --installed | grep -q '^rustfmt' || rustup component add rustfmt
	rustup component list --installed | grep -q '^clippy'  || rustup component add clippy
	command -v cargo-llvm-cov >/dev/null 2>&1 || { rustup component add llvm-tools-preview; cargo install cargo-llvm-cov --locked; }
	command -v cargo-audit   >/dev/null 2>&1 || cargo install cargo-audit --locked
	command -v cargo-outdated >/dev/null 2>&1 || cargo install cargo-outdated --locked
	command -v cargo-udeps   >/dev/null 2>&1 || cargo install cargo-udeps --locked
	rustup toolchain list | grep -q '^nightly' || rustup toolchain install nightly
	rustup +nightly component add miri || true
	rustup +nightly component add rust-src || true
	cargo +nightly miri setup || true

# ---- カバレッジ ---------------------------------------------------------------
coverage: ## カバレッジ (HTML, JSON, LCOV)
	cargo llvm-cov --workspace --all-features --html
	cargo llvm-cov report --json --output-path coverage.json
	cargo llvm-cov report --lcov --output-path target/llvm-cov/lcov.info
	@echo "HTML: target/llvm-cov/html/index.html"
	@echo "JSON: coverage.json"
	@echo "LCOV: target/llvm-cov/lcov.info"

# ---- 健康診断 & 解析 ----------------------------------------------------------
audit: ## 既知脆弱性チェック
	cargo audit

outdated: ## 依存の更新状況
	cargo outdated

udeps: ## 未使用依存（nightly）
	cargo +nightly udeps --workspace

miri: ## 未定義動作の検査（nightly）
	cargo +nightly miri test

bench: ## ベンチ（criterion 想定）
	cargo bench

# ---- チェック & CI フロー -----------------------------------------------------
check: fmt clippy test ## フォーマット＋Lint＋テスト
	@echo "✅ コードチェック (fmt → clippy → test) 完了"

full_local: add-tools clean fmt clippy test release audit outdated udeps miri doc coverage ## フルローカルビルド
	@echo "✅ フルローカルビルド (clean → fmt → clippy → test → release → audit → outdated → udeps → miri → doc → coverage) 完了"

ci: clean fmt-check clippy ## CI: クリーンビルド + fmt-check + clippy + test + release
	RUSTFLAGS="-D warnings" cargo test  --workspace --all-features --frozen --locked --verbose
	RUSTFLAGS="-D warnings" cargo build --workspace --all-features --frozen --locked --release
	@echo "✅ CIフロー (clean → fmt-check → clippy → test → release) 完了"
