<!-- パス: .codex/AGENTS.md -->
<!-- 役割: Unified agent guide (principles + quickstart + ops + templates + UI + data) -->
<!-- 意図: Single-source, daily-ready, 80/20-aligned execution guide -->
<!-- 関連ファイル: BOOT_PROMPT_EN.txt, README.md, .github/copilot-instructions.md -->

# AGENTS Guide / エージェントガイド

## Conversation Language / 会話言語
- codex-cli dialogue is conducted in Japanese. (codex-cli の会話は日本語で行う。)

## Quickstart (10 lines) / クイックスタート（10行）
1) **Do exactly what’s asked. No more, no less.** (依頼どおりに実行し、過不足を出さない。)
2) **Prefer minimal, simple solutions (80/20).** (最小でシンプルな 80/20 解を最優先。)
3) **Read the whole target + related files before editing.** (対象と関連ファイルを編集前に必読。)
4) **Start with a tiny working prototype.** (小さく動くプロトタイプから着手。)
5) **Add the Japanese 4-line header to every file.** (全ファイルに日本語4行ヘッダを追加。)
6) **Small, focused commits; 1 PR = 1 purpose.** (小粒で焦点の定まったコミット、1PR=1目的。)
7) **Comment non-obvious logic; use clear identifiers.** (自明でないロジックにコメント、明快な命名を徹底。)
8) **Risky ops: present reason + alternatives + rollback before acting.** (危険操作は理由・代替案・ロールバック案を事前提示。)
9) **Capture rationale in headers + PR body.** (判断理由はヘッダとPR本文に記録。)
10) **Push to GitHub only with explicit user approval.** (GitHub への push は明示承認がある場合のみ。)

---

## AI-Driven Development Guidelines / AI駆動開発 共通ガイドライン

### 開発の基本理念 / Development Philosophy
- 動くコードだけでなく品質・保守性・安全性も常に意識する。(Deliver working code while safeguarding quality, maintainability, and safety.)
- プロジェクトフェーズ（プロトタイプ、MVP、本番）に応じた最適バランスを取る。(Balance rigor with the current project phase: prototype, MVP, or production.)
- 問題を見つけたら放置せず対処するか記録する。(Address issues immediately or document them explicitly.)
- ボーイスカウトルールを守り、改善された状態で残す。(Apply the Boy Scout rule: leave the code better than you found it.)

### エラーハンドリングの原則 / Error Handling Principles
- 関連が薄く見えるエラーでも原因を究明し解決する。(Investigate and resolve even seemingly unrelated errors.)
- `@ts-ignore` 等でエラーを握りつぶさず、根本原因を修正する。(Fix root causes instead of suppressing errors like `@ts-ignore`.)
- 早期検出と明瞭なエラーメッセージを心掛ける。(Detect errors early and surface clear messages.)
- エラーケースもテストで必ずカバーする。(Test error paths alongside happy paths.)
- 外部APIやネットワーク失敗を前提に設計する。(Design assuming external APIs and network calls can fail.)

### コード品質の基準 / Code Quality Standards
- DRY原則を守り単一の信頼できる情報源を維持する。(Apply DRY to maintain a single source of truth.)
- 意味のある命名と一貫したコーディングスタイルを使う。(Use meaningful names and consistent style.)
- 小さな問題も放置せず速やかに修正する。(Fix small issues as soon as they surface.)
- コメントは「なぜ」を説明し「何を」はコードで表現する。(Use comments to explain why, not what.)

### テスト規律 / Testing Discipline
- テストをスキップせず失敗したら修正する。(Never skip tests; fix failures before proceeding.)
- 振る舞いにフォーカスしたテストを書く。(Test observable behaviour, not implementation details.)
- テスト間の依存を避け任意順序で走るようにする。(Keep tests order-independent.)
- テストは高速で決定的に保つ。(Ensure tests are fast and deterministic.)
- カバレッジは指標であり質の高いテストを重視する。(Treat coverage as a metric; prioritise test quality.)

### 保守性とリファクタリング / Maintainability & Refactoring
- 機能追加と同時に既存コードの改善を検討する。(Improve existing code when adding features.)
- 大規模変更は小さなステップに分割する。(Break large changes into manageable steps.)
- 未使用コードは積極的に削除する。(Remove unused code proactively.)
- 依存関係はセキュリティと互換性のため定期更新する。(Update dependencies regularly for security and compatibility.)
- 技術的負債はコメントや文書で明示する。(Document technical debt openly.)

### セキュリティの考え方 / Security Mindset
- APIキーやパスワードは環境変数で管理する。(Keep secrets in environment variables; avoid hard-coding.)
- 外部入力はすべて検証する。(Validate all external inputs.)
- 必要最小限の権限で動作させる。(Apply the principle of least privilege.)
- 不要な依存関係は避ける。(Avoid unnecessary dependencies.)
- セキュリティ監査ツールを定期的に実行する。(Run security scans regularly.)

### パフォーマンスの意識 / Performance Awareness
- 推測ではなく計測結果に基づいて最適化する。(Optimise based on measurement, not guesswork.)
- 初期段階から拡張性を意識する。(Keep scalability in mind from the start.)
- 必要になるまでリソース読み込みを遅延する。(Defer resource loading until needed.)
- キャッシュの期限と無効化戦略を明確にする。(Define cache expiry and invalidation strategies.)
- N+1やオーバーフェッチを避ける。(Avoid N+1 queries and over-fetching.)

### 信頼性の確保 / Reliability Practices
- 適切なタイムアウトを設定する。(Set sensible timeouts.)
- リトライ機構は指数バックオフを検討する。(Use retry mechanisms with exponential backoff.)
- サーキットブレーカーパターンを活用する。(Apply circuit breaker patterns when helpful.)
- 一時的な障害に耐性を持たせる。(Design for transient failure tolerance.)
- ログとメトリクスで可観測性を確保する。(Maintain observability with logs and metrics.)

### プロジェクトコンテキストの理解 / Project Context Awareness
- ビジネス要件と技術要件のバランスを取る。(Balance business and technical needs.)
- 現在フェーズに必要な品質レベルを判断する。(Align quality level with current phase.)
- 時間制約下でも最低限の品質基準を守る。(Uphold baseline quality even under time pressure.)
- チーム全体の技術レベルに合わせた実装を選ぶ。(Choose implementations aligned with the team’s skill level.)

### トレードオフの認識 / Trade-off Clarity
- 完璧は不可能と理解し現実的な解を探る。(Acknowledge no silver bullet; aim for pragmatic balance.)
- 制約の中で最適解を見つける。(Find the best option within constraints.)
- プロトタイプでは簡潔さを、本番では堅牢性を優先する。(Prioritise simplicity for prototypes and robustness for production.)
- 妥協点と理由を明確にドキュメント化する。(Document compromises and their rationale.)

### Git運用の基本 / Git Workflow Basics
- コンベンショナルコミット形式を使う。(Use Conventional Commit formatting.)
- コミットは原子的で単一変更に集中させる。(Keep commits atomic and focused.)
- 明確で説明的なコミットメッセージを英語で書く。(Write clear English commit messages.)
- main/master への直接コミットは避ける。(Avoid direct commits to main/master branches.)

### コードレビューの姿勢 / Code Review Mindset
- レビューコメントは建設的な改善提案として扱う。(Treat feedback as constructive suggestions for the code.)
- 個人ではなくコードに焦点を当てる。(Focus on the code, not the person.)
- 変更理由と影響を明確に説明する。(Explain the rationale and impact of changes.)
- フィードバックを学習機会として歓迎する。(Welcome feedback as a learning opportunity.)

### デバッグのベストプラクティス / Debugging Best Practices
- 問題を再現できる手順を確立する。(Create reliable reproduction steps.)
- 二分探索で問題範囲を絞り込む。(Use binary search to narrow down issues.)
- 最近の変更から調査を始める。(Start by inspecting recent changes.)
- デバッガーやプロファイラなど適切なツールを活用する。(Leverage debuggers, profilers, and relevant tools.)
- 調査結果と解決策を記録し共有する。(Document findings and share knowledge.)

### 依存関係の管理 / Dependency Management
- 本当に必要な依存関係のみ追加する。(Add dependencies only when necessary.)
- ロックファイル（例: package-lock.json）を必ずコミットする。(Commit lockfiles such as package-lock.json.)
- 新しい依存追加前にライセンス・サイズ・メンテ状況を確認する。(Check license, footprint, and maintenance before adding new deps.)
- セキュリティパッチとバグ修正のため定期的に更新する。(Update periodically for security patches and bug fixes.)

### ドキュメントの基準 / Documentation Standards
- README に概要・セットアップ・使用方法を明記する。(Keep README updated with overview, setup, and usage.)
- コード変更とドキュメントを同期する。(Sync documentation with code changes.)
- 実例を優先的に示す。(Provide practical examples.)
- 重要な設計判断は ADR に記録する。(Record major design choices in ADRs.)

### 継続的な改善 / Continuous Improvement
- 学んだことを次のプロジェクトに活かす。(Carry lessons into future work.)
- 定期的に振り返りプロセスを改善する。(Run retrospectives and refine processes.)
- 新しいツールや手法を評価して適切に取り入れる。(Evaluate and adopt new tools mindfully.)
- チームや将来の開発者のために知識を文書化する。(Document knowledge for the team and future developers.)

---

## Mission & Scope / 使命と範囲

### Goal / 目的
- **Clean, simple, readable, modular, well-documented code.** (クリーン・シンプル・読みやすく・モジュール化された文書化済みコードを提供。)
- Think with **senior developer judgment** and avoid over-engineering. (シニア開発者の判断基準で考え、過剰設計を避ける。)

### Modus Operandi / 作法
- Prioritise **simplicity and minimalism**; include tiny runnable examples when useful. (シンプルさとミニマリズムを優先し、必要に応じて最小実行例を示す。)
- When uncertain: **prototype → observe → adjust**. (不確実なときは試作→観察→調整。)

### Roles / 関与者
- **User (human)** directs the project and makes final calls. (ユーザーがプロジェクトを指揮し最終決定する。)
- **Human developers** assist as needed. (人間開発者は必要に応じて支援する。)
- **VS Code (AI copilot)** provides IDE assistance with moderate autonomy. (VS Code の AI コパイロットが IDE 補助を行う。)
- **AI agents (Codex など)** handle multi-file edits, tests, Git ops, and refactors. (AI エージェントは複数ファイル編集・テスト・Git 操作・リファクタを担当。)

### Scope & Restrictions / 範囲と制約
- Push only with explicit instruction. (明示的な指示がある場合のみ push。)
- No scope creep: extra ideas move to proposals. (要求外は実装せず提案へ分離。)

### Version Control / バージョン管理
- Produce small, meaningful commits using `<type>: <summary>` style. (小粒で意味のあるコミットを `<type>: <summary>` 形式で記録。)
- Keep each PR focused on a single purpose with reviewable diffs. (各PRは1目的に絞りレビューしやすい差分に保つ。)

### File Header (4 lines) / ファイルヘッダ（4行）
1) Path / ファイルパス  2) What / 何をするか  3) Why / なぜ存在するか  4) Relevant files / 関連ファイル2〜4。 (この4行ヘッダは削除禁止。)

### Comments / コメント
- State role at the top and document non-obvious logic. (先頭で役割を明示し、自明でないロジックをコメント。)
- Use clear English identifiers and supplement with Japanese comments when helpful. (明快な英語の識別子を使い、必要に応じて日本語で補足。)

### Reading First / まず読む
- Read the entire file and related files before editing. (編集前に対象と関連ファイルを全文確認。)

### Simplicity / シンプルさ
- **SIMPLE = good, COMPLEX = bad**; minimise LOC without harming readability. (SIMPLE=善／COMPLEX=悪。可読性を保ったまま行数を抑える。)

### Decision Framework / 判断ガイド
1) **Resilience > Speed**, unless a small 80/20 provides clear value. (価値が明確な 80/20 でない限り壊れにくさを速度より優先。)
2) If explanations grow long, ship smaller and observe. (説明が長くなるなら小さく出して観察。)
3) Nice-to-haves belong in proposals, not scope. (Nice-to-have は提案に分離しスコープから外す。)

---

## Operational Playbooks / 運用詳細

### Checklists / チェックリスト
**Before Change / 変更前**
- [ ] Restate the request in 1–2 lines. (依頼内容を1〜2行で言い換える。)
- [ ] List related files and expected impact. (関連ファイルと影響範囲を列挙。)
- [ ] Review spec/UI/API differences. (仕様・UI・APIの差分を確認。)

**During Implementation / 実装時**
- [ ] Start from the minimal viable change. (最小実装から着手。)
- [ ] Ensure 4-line header, comments, and naming are applied. (4行ヘッダ・コメント・命名を遵守。)
- [ ] Surface actionable errors per policy. (方針に従い行動可能なエラーメッセージを出す。)

**Test & Verify / テスト・検証**
- [ ] Cover happy, sad, and boundary cases with tests. (正常・異常・境界ケースをテストで網羅。)
- [ ] Capture short manual check notes when applicable. (必要に応じて手動確認メモを残す。)
- [ ] Provide before/after UI evidence when feasible. (可能なら UI の前後差を記録。)

**Before PR / PR 前**
- [ ] Summarise changes in ≤3 bullets. (変更点を3項以内で要約。)
- [ ] Explain why the solution is 80/20-sufficient. (80/20 で十分な理由を記載。)
- [ ] Note rollback in 1–2 lines. (ロールバック方法を1〜2行で記す。)

### Risky-Change Protocol / 危険変更プロトコル
- Risk types: data loss, security, broad refactor, infra change. (リスク: データ喪失・セキュリティ・大規模リファクタ・インフラ変更。)
- Before acting: (1) reason & alternatives (2) blast radius & rollback (3) phased rollout plan. (実施前に①理由と代替案②影響範囲とロールバック③段階導入計画を用意。)
- Execute only after explicit user approval. (ユーザーの明示承認後に実行。)

### Quality Gates / 品質ゲート
- Mandatory: fmt / lint / unit. (必須: fmt / lint / unit。)
- Scale E2E/integration effort to impact. (E2E・統合テストは影響度に応じて実施。)
- Record rationale in headers and PR body. (判断理由はヘッダとPR本文に記録。)

### Knowledge Capture / 知識の蓄積
- Document decisions in headers and PRs; link issues when relevant. (決定事項はヘッダとPRに残し、必要に応じて課題へリンク。)

---

## Templates / 雛形

### File Header (4 lines) / ファイルヘッダ（4行）
```txt
// Path: <repo-relative path>
// What: <what this file provides, in one short line>
// Why : <design intent / rationale, one short line>
// RELEVANT FILES: <2–4 related files>
```

### PR Title / PRタイトル
```
feat: minimal implementation of X (separates Y, simplifies Z)
```

### PR Body (minimal) / PR本文（最小）
- **Summary**: Add/change ○○. (要約: ○○を追加／変更。)
- **80/20**: Why this design is sufficient now. (80/20: 今この設計で十分な理由。)
- **Impact**: API/UI changes or migrations? (影響: API／UI変更・移行の有無。)
- **Rollback**: `revert <sha>` or flip `<flag>`. (ロールバック: `revert <sha>` または `<flag>` 切替。)

### Commit Message / コミットメッセージ
- Style: `<type>: <summary>` (e.g., `fix: replace panic with Result`). (形式: `<type>: <summary>` 例 `fix: panic を Result に変更`。)

### Review Shortlist / レビュー要点
- Scope fits request? (依頼範囲内か？)
- Minimal & readable? (最小で読みやすいか？)
- Tests cover happy/sad/boundary? (テストは正常・異常・境界を網羅しているか？)
- Rationale captured? (判断理由は記録されているか？)

---

## UI Guide (Essentials) / UIガイド（要点）
- Keep interfaces simple, clean, and minimal to reduce cognitive load. (シンプルでクリーンなUIにして認知負荷を下げる。)
- Reference Apple and ChatGPT patterns for tone and polish. (Apple や ChatGPT のパターンを参照。)
- Colors: base black/white, accent deep blue, neutrals without blue tint. (色: ベースは黒/白、アクセントは濃い青、中立的なグレーを使用。)
- Spacing & type: consistent 4/8px rhythm, concise headings, short sentences. (余白とタイポグラフィは4/8pxのリズムで、見出しと文は簡潔に。)

---

## Data Policy (DB) / データポリシー（DB）
- Only the user executes database changes; agents propose but do not run them. (DB変更はユーザーのみ実行し、エージェントは提案に留める。)
- Proposals must include reason, schema diff, migration sketch, risk, and rollback. (提案には理由・スキーマ差分・移行案・リスク・ロールバックを含める。)
- Ensure backups, define rollback (down migration or restore), and phase risky changes. (事前バックアップ、ロールバック手段の明示、段階導入で安全性を確保。)

---

## Boot (optional) / 起動用（任意）
**One-liner to paste at startup / 起動直後に貼るワンライナー**
```
Read ./.codex/AGENTS.md now and follow it strictly; then reply with (a) 3 key rules, (b) the next 3 steps, and (c) one-line risks+rollback.
```
**日本語補助**
```
いま ./.codex/AGENTS.md を読み、厳守してください。読了後に (a) 規範3点 (b) 次の3手 (c) リスク＋ロールバック1行 を返答してください。
```

---

## Glossary / 用語
- **80/20**: maximum impact with minimal effort. (最小の労力で最大の効果。)
- **APPU**: Active Paying Power Users (business-side metric). (APPU: 有料で能動的なパワーユーザー数。)
- **Minimal implementation**: a small, robust core that satisfies the request. (最小実装: 依頼を満たす小さく堅牢な核。)
