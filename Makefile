# ==============================================================================
# このRustプロジェクト用 Makefile
#
# 使い方:
#   make          - このヘルプメッセージを表示します。
#   make <ターゲット名> - 特定のタスクを実行します。
#
# 事前準備 (一度だけ実行):
#   カバレッジ測定のターゲットを使うには、事前に以下のツールをインストールしてください。
#   $ rustup component add llvm-tools-preview
#   $ cargo install cargo-llvm-cov --locked
# ==============================================================================

# makeコマンドの引数が指定されなかった場合、デフォルトで'help'ターゲットを実行します。
.DEFAULT_GOAL := help

# .PHONYターゲット: ファイル名ではないターゲットを宣言します。
.PHONY: help \
	clean fmt clippy test build doc \
	coverage coverage-json coverage-summary \
	check ci pipeline

# --- メインターゲット ---
help: ## このヘルプメッセージを表示します
	@echo "使い方: make [ターゲット名]"
	@echo ""
	@echo "利用可能なターゲット:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'

clean: ## targetディレクトリとビルド成果物を削除します
	@echo ">> ビルド成果物をクリーンアップ中..."
	@cargo clean

fmt: ## ソースコードをフォーマットします
	@echo ">> コードをフォーマット中..."
	@cargo fmt

clippy: ## Clippyでコードをリントします (警告をエラーとして扱います)
	@echo ">> Clippyでコードをリント中..."
	@cargo clippy --all-targets -- -D warnings

test: ## すべてのテストを詳細ログ付きで実行します
	@echo ">> テストを実行中..."
	@cargo test --all --verbose

build: ## メインのバイナリをビルドします
	@echo ">> バイナリ 'typelang-repl' をビルド中..."
	@cargo build --release

doc: ## ドキュメントを生成します (依存クレートは除く)
	@echo ">> ドキュメントを生成中..."
	@RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

# --- カバレッジ関連ターゲット ---
coverage: ## HTML形式のカバレッジレポートを生成します
	@echo ">> HTMLカバレッジレポートを生成中..."
	@cargo llvm-cov --workspace --html
	@echo "HTMLレポートが target/llvm-cov/html/index.html に生成されました"

coverage-json: ## JSON形式のカバレッジレポートを生成します (CIでのチェック用)
	@echo ">> JSONカバレッジレポートを生成中..."
	@cargo llvm-cov report --json --output-path coverage.json
	@echo "JSONレポートが coverage.json に生成されました"

coverage-summary: ## カバレッジの概要をコンソールに表示します
	@echo ">> カバレッジの概要を表示中..."
	@cargo llvm-cov --workspace --summary-only

# --- 複合ターゲット ---
check: fmt clippy test build ## フォーマット、リント、テスト、ビルドを順に実行します
	@echo "✅ すべての品質チェックとビルドが完了しました！"

ci: clean check ## CI環境で実行するタスク
	@echo "✅ CI用のタスクが完了しました！"

pipeline: ci doc coverage coverage-json coverage-summary ## プロジェクトの全主要タスクを順に実行します
	@echo "✅ 全てのパイプライン処理が完了しました！"