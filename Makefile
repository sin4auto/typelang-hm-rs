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
  check full_local ci \
  serena-summarize diffpack code-meta test-brief build-brief model-pack

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

# ---- Serena 要約 --------------------------------------------------------------
serena-summarize: ## Serena の要約(.serena)を更新
	# Codex CLI から Serena MCP を呼ぶ
	codex mcp run serena summarize || true
	@echo "=> .serena/ を更新しました（MODEL_INPUT.md / SYMBOLS など）"

# ---- トークン節約支援 ---------------------------------------------------------
diffpack: ## 直近差分の要約（ファイル一覧/短統計/最近ログ）
	@mkdir -p .summ
	@{ git rev-parse --is-inside-work-tree >/dev/null 2>&1 && \
	   git fetch --quiet origin || true; } || true
	ORIGIN_REF ?= origin/main
	@git diff --name-only $(ORIGIN_REF)...HEAD > .summ/CHANGED_FILES.txt || true
	@git diff --shortstat  $(ORIGIN_REF)...HEAD > .summ/DIFF_SHORTSTAT.txt || true
	@git log --oneline -n 30 > .summ/RECENT_LOG.txt
	@echo "=> .summ/CHANGED_FILES.txt / DIFF_SHORTSTAT.txt / RECENT_LOG.txt を生成"

code-meta: ## 公開APIやシンボルを軽量抽出（ctags/rg があれば活用）
	@mkdir -p .summ
	@if command -v ctags >/dev/null 2>&1; then \
	  ctags -R --fields=+n --languages=Rust -f .summ/SYMBOLS.tags src || true ; \
	else echo "WARN: ctags が見つかりません（SYMBOLS.tagsはスキップ）"; fi
	@if command -v rg >/dev/null 2>&1; then \
	  rg -n "^(pub (fn|struct|enum|trait)|mod\s+)" -S src > .summ/PUBLIC_API.tsv || true ; \
	else echo "WARN: ripgrep(rg) が見つかりません（PUBLIC_API.tsvはスキップ）"; fi
	@echo "=> .summ/SYMBOLS.tags / PUBLIC_API.tsv を生成（存在すれば）"

test-brief: ## テストの失敗要点を抽出（最大200行）
	cargo test --workspace --all-features -q || true
	@if command -v rg >/dev/null 2>&1; then \
	  rg -n "FAILED|error|panicked" -S target | tail -n 200 > .summ/TEST_ERRORS.txt || true ; \
	else \
	  grep -RniE "FAILED|error|panicked" target 2>/dev/null | tail -n 200 > .summ/TEST_ERRORS.txt || true ; \
	fi
	@echo "=> .summ/TEST_ERRORS.txt（最大200行）"

build-brief: ## ビルドエラー・警告の要点（最大200行）
	cargo build --workspace --all-features -q || true
	@if command -v rg >/dev/null 2>&1; then \
	  rg -n "error\[|warning:" -S target | tail -n 200 > .summ/BUILD_ERRORS.txt || true ; \
	else \
	  grep -RniE "error\\[|warning:" target 2>/dev/null | tail -n 200 > .summ/BUILD_ERRORS.txt || true ; \
	fi
	@echo "=> .summ/BUILD_ERRORS.txt（最大200行）"

model-pack: serena-summarize diffpack code-meta test-brief build-brief ## .serena + .summ をZip
	@mkdir -p .summ
	@zip -q -r .summ/model_pack.zip .summ .serena 2>/dev/null || true
	@echo "=> .summ/model_pack.zip を生成（MODEL_INPUT.md/差分/短いログを同梱）"

# ---- チェック & CI フロー -----------------------------------------------------
check: fmt clippy test ## フォーマット + Lint + テスト
	@echo "✅ コードチェック (fmt → clippy → test) 完了"

full_local: clean fmt clippy test doc audit outdated coverage release udeps miri ## clean + フォーマット + Lint + テスト + ドキュメント + 健康診断 + カバレッジ + リリースビルド + 未使用依存 + 未定義動作検査
	@echo "✅ フルローカルビルド (clean → fmt → clippy → test → doc → audit → outdated → coverage → release → udeps → miri) 完了"
