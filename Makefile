# ==============================================================================
# Rust プロジェクト Makefile（変数なし・check→ci 順）
# ==============================================================================

.DEFAULT_GOAL := help

SHELL := /usr/bin/bash
.SHELLFLAGS := -eu -o pipefail -c
.ONESHELL:
.EXPORT_ALL_VARIABLES:

.PHONY: \
  help clean \
  fmt fmt-check clippy \
  test build release \
  doc doc-open coverage \
  audit outdated udeps miri bench \
  check full_local ci add-tools

help: ## このヘルプを表示
	@grep -E '^[a-zA-Z0-9_-]+:.*?## ' $(MAKEFILE_LIST) | \
	  awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'

# ---- 基本 ---------------------------------------------------------------------
clean: ## target などを削除
	cargo clean

fmt: ## 整形を実行（ローカル用）
	cargo fmt --all

fmt-check: ## 整形済みかを検査（CI向け）
	cargo fmt --all -- --check

clippy: ## Clippy（警告=エラー）
	cargo clippy --workspace --all-targets --all-features -- -D warnings

test: ## テスト（ワークスペース全体）
	cargo test --workspace --all-features --verbose

build: ## デバッグビルド（ローカル用）
	cargo build --workspace --all-features

release: ## リリースビルド（ローカル用）
	cargo build --workspace --all-features --release

doc: ## ドキュメント生成（警告=エラー）
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features

doc-open: doc ## 生成後にブラウザで開く
	html="target/doc/index.html"; \
	if command -v wslview >/dev/null 2>&1; then wslview "$$html"; \
	elif command -v xdg-open >/dev/null 2>&1; then xdg-open "$$html"; \
	elif command -v open >/dev/null 2>&1; then open "$$html"; \
	else echo "open $$html"; fi

# ---- ツール導入（未導入時のみ） -----------------------------------------------
add-tools: ## llvm-cov/cargo-audit/cargo-outdated/cargo-udeps/miri を未導入なら導入
	command -v cargo-llvm-cov >/dev/null 2>&1 || { rustup component add llvm-tools-preview; cargo install cargo-llvm-cov --locked; }
	command -v cargo-audit    >/dev/null 2>&1 || cargo install cargo-audit
	command -v cargo-outdated >/dev/null 2>&1 || cargo install cargo-outdated
	command -v cargo-udeps    >/dev/null 2>&1 || cargo +nightly install cargo-udeps
	rustup +nightly component add miri

# ---- カバレッジ ---------------------------------------------------------------
coverage: ## カバレッジ (HTML, JSON, LCOV)
	RUSTFLAGS="-C link-dead-code" cargo llvm-cov --workspace --html
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

ci: clean fmt-check clippy test release ## CI: クリーンビルド + fmt-check + clippy + test + release
	cargo build --workspace --all-features --frozen --locked --release
	cargo test  --workspace --all-features --frozen --locked --verbose
	@echo "✅ CIフロー (clean → fmt-check → clippy → test → release) 完了"
