# ==============================================================================
# Rustプロジェクト用 Makefile
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
	clean fmt clippy test debug_build release_build doc \
	coverage \
	debug release ci full_local

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

debug_build: ## デバッグビルドを行います (最適化なし、開発用)
	@echo ">> デバッグビルド中..."
	@cargo build

release_build: ## リリースビルドを行います (最適化あり、本番用)
	@echo ">> リリースビルド中..."
	@cargo build --release

doc: ## ドキュメントを生成します (依存クレートは除く)
	@echo ">> ドキュメントを生成中..."
	@RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

# --- カバレッジ関連ターゲット (エラー修正版) ---
coverage: ## カバレッジレポート (HTML, JSON) を生成します
	@echo ">> カバレッジ測定用のテストを実行し、HTMLレポートを生成中..."
	@cargo llvm-cov --workspace --html
	@echo "HTMLレポートが target/llvm-cov/html/index.html に生成されました"
	@echo ">> 生成されたデータを元に、JSONレポートを生成中..."
	@cargo llvm-cov report --json --output-path coverage.json
	@echo "JSONレポートが coverage.json に生成されました"

# --- 複合ターゲット ---
debug: fmt clippy test debug_build ## フォーマット、リント、テスト、デバッグビルドを順に実行します
	@echo "✅ すべての品質チェックとデバッグビルドが完了しました！"

release: fmt clippy test release_build ## フォーマット、リント、テスト、リリースビルドを順に実行します
	@echo "✅ すべての品質チェックとリリースビルドが完了しました！"

ci: clean release ## CIで実行するタスク
	@echo "✅ 全てのCI処理が完了しました！"

full_local: clean fmt clippy release_build doc coverage ## ドキュメント生成からカバレッジテストまでのフルパイプライン 
	@echo "✅ ドキュメント生成からカバレッジテストまでのフルパイプライン処理が完了しました！"