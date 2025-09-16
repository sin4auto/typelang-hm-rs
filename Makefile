.DEFAULT_GOAL := help

SHELL := /usr/bin/bash
.SHELLFLAGS := -eu -o pipefail -c
.ONESHELL:
.EXPORT_ALL_VARIABLES:

.PHONY: \
  help add-tools clean \
  fmt fmt-check clippy \
  test build release \
  doc doc-open coverage \
  audit outdated udeps miri bench \
  check full_local ci

help: ## このヘルプを表示
	@grep -E '^[a-zA-Z0-9_-]+:.*?## ' $(MAKEFILE_LIST) | \
	  awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2}'

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

build: ## デバッグビルド
	cargo build --workspace --all-features --frozen --locked

release: ## リリースビルド
	cargo build --workspace --all-features --frozen --locked --release

doc: ## ドキュメント生成（警告=エラー）
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features

doc-open: doc ## 生成後にブラウザで開く
	html="target/doc/index.html"; \
	if command -v wslview >/dev/null 2>&1; then wslview "$$html"; \
	elif command -v xdg-open >/dev/null 2>&1; then xdg-open "$$html"; \
	elif command -v open >/dev/null 2>&1; then open "$$html"; \
	else echo "open $$html"; fi

# ---- ツール導入（未導入時のみ） -----------------------------------------------
add-tools: ## rustfmt/clippy/llvm-cov を未導入なら導入
	command -v rustfmt >/dev/null 2>&1 || rustup component add rustfmt
	command -v cargo-clippy >/dev/null 2>&1 || rustup component add clippy
	command -v cargo-llvm-cov >/dev/null 2>&1 || { rustup component add llvm-tools-preview; cargo install cargo-llvm-cov --locked; }

# ---- カバレッジ ---------------------------------------------------------------
coverage: add-tools ## カバレッジ測定＋レポート生成（HTML/JSON/LCOV）
	cargo llvm-cov clean --workspace
	cargo llvm-cov --workspace --html
	cargo llvm-cov report --json --output-path coverage.json
	cargo llvm-cov report --lcov --output-path target/llvm-cov/lcov.info
	@echo "HTML: target/llvm-cov/html/index.html"
	@echo "JSON: coverage.json"
	@echo "LCOV: target/llvm-cov/lcov.info"

# ---- 健康診断 & 解析 ----------------------------------------------------------
audit: ## 既知脆弱性チェック
	command -v cargo-audit >/dev/null 2>&1 || cargo install cargo-audit
	cargo audit

outdated: ## 依存の更新状況
	command -v cargo-outdated >/dev/null 2>&1 || cargo install cargo-outdated
	cargo outdated

udeps: ## 未使用依存（nightly）
	rustup toolchain install nightly --no-self-update || true
	command -v cargo-udeps >/dev/null 2>&1 || cargo +nightly install cargo-udeps --locked
	cargo +nightly udeps --workspace

miri: ## 未定義動作の検査（nightly）
	rustup toolchain install nightly --no-self-update || true
	rustup +nightly component add miri
	cargo +nightly miri test

bench: ## ベンチ（criterion 想定）
	cargo bench

# ---- チェック & CI フロー -----------------------------------------------------
check: fmt clippy test ## フォーマット + Lint + テスト
	@echo "✅ コードチェック (fmt → clippy → test) 完了"

full_local: clean fmt clippy test doc audit outdated coverage release udeps miri ## clean + フォーマット + Lint + テスト + ドキュメント + 健康診断 + カバレッジ + リリースビルド + 未使用依存 + 未定義動作検査
	@echo "✅ フルローカルビルド (clean → fmt → clippy → test → doc → audit → outdated → coverage → release → udeps → miri) 完了"

ci: fmt-check clippy test release ## CI: format check + Lint + テスト + リリースビルド
	@echo "✅ CIフロー (fmt-check → clippy → test → release) 完了"
