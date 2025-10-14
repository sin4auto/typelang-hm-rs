// パス: src/repl/line_editor.rs
// 役割: Terminal line editor handling history and cursor movement
// 意図: Provide portable interactive input for the REPL
// 関連ファイル: src/repl/cmd.rs, src/repl/printer.rs, src/repl/util.rs
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

/// 行入力が返す 3 種類の結果を表す列挙体。
pub enum ReadResult {
    Line(String),
    Eof,
    Interrupted,
}

/// 履歴付きの行編集を提供する簡易ラインエディタ。
pub struct LineEditor {
    history: History,
}

/// `LineEditor` のパブリックな操作群をまとめた実装。
impl LineEditor {
    /// 保存済みの履歴を読み込み、新しいエディタを構築する。
    pub fn new() -> Self {
        Self {
            history: History::load(),
        }
    }

    /// プロンプトを出力し、1 行分の入力または制御シグナルを取得する。
    pub fn read_line(&mut self, prompt: &str) -> io::Result<ReadResult> {
        #[cfg(unix)]
        {
            self.read_line_unix(prompt)
        }
        #[cfg(not(unix))]
        {
            self.read_line_fallback(prompt)
        }
    }

    /// 入力文字列を履歴へ追加し、重複や空行を除外する。
    pub fn add_history(&mut self, entry: &str) {
        self.history.add(entry);
    }

    /// 現在の履歴内容を永続ストレージへ書き出す。
    pub fn save_history(&self) -> io::Result<()> {
        self.history.save()
    }
}

/// 既定の初期化は `new` を呼び出して共通化する。
impl Default for LineEditor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(unix))]
impl LineEditor {
    /// Raw モードが利用できない環境向けのフォールバック読み取り。
    fn read_line_fallback(&mut self, prompt: &str) -> io::Result<ReadResult> {
        let mut stdout = io::stdout();
        write!(stdout, "{}", prompt)?;
        stdout.flush()?;
        let mut line = String::new();
        let bytes = io::stdin().read_line(&mut line)?;
        if bytes == 0 {
            return Ok(ReadResult::Eof);
        }
        if line.ends_with('\n') {
            line.pop();
        }
        if line.ends_with('\r') {
            line.pop();
        }
        Ok(ReadResult::Line(line))
    }
}

#[cfg(unix)]
impl LineEditor {
    /// UNIX 端末を Raw モードに切り替えて対話入力を処理する。
    #[allow(unexpected_cfgs)]
    #[cfg_attr(coverage, coverage(off))]
    fn read_line_unix(&mut self, prompt: &str) -> io::Result<ReadResult> {
        let _raw = RawMode::new()?;
        let mut stdout = io::stdout();
        write!(stdout, "{}", prompt)?;
        stdout.flush()?;

        let stdin = io::stdin();
        let mut stdin = stdin.lock();
        let mut session = EditorSession::new(&self.history);
        loop {
            let mut byte = [0u8; 1];
            if stdin.read(&mut byte)? == 0 {
                return Ok(ReadResult::Eof);
            }
            match interpret_action(byte[0], &mut stdin)? {
                EditAction::Submit => {
                    write!(stdout, "\r\n")?;
                    stdout.flush()?;
                    return Ok(ReadResult::Line(session.into_string()));
                }
                EditAction::Interrupt => {
                    write!(stdout, "^C\r\n")?;
                    stdout.flush()?;
                    return Ok(ReadResult::Interrupted);
                }
                EditAction::Eof => {
                    if session.is_empty() {
                        return Ok(ReadResult::Eof);
                    }
                }
                EditAction::DeleteLeft => {
                    if session.delete_left() {
                        refresh_line(&mut stdout, prompt, session.buffer(), session.cursor())?;
                    }
                }
                EditAction::MoveLeft => {
                    if session.move_left() {
                        refresh_line(&mut stdout, prompt, session.buffer(), session.cursor())?;
                    }
                }
                EditAction::MoveRight => {
                    if session.move_right() {
                        refresh_line(&mut stdout, prompt, session.buffer(), session.cursor())?;
                    }
                }
                EditAction::HistoryPrev => {
                    if session.history_prev() {
                        refresh_line(&mut stdout, prompt, session.buffer(), session.cursor())?;
                    }
                }
                EditAction::HistoryNext => {
                    if session.history_next() {
                        refresh_line(&mut stdout, prompt, session.buffer(), session.cursor())?;
                    }
                }
                EditAction::InsertChar(ch) => {
                    session.insert_char(ch);
                    refresh_line(&mut stdout, prompt, session.buffer(), session.cursor())?;
                }
                EditAction::Ignore => {}
            }
        }
    }
}

/// 先頭バイトと後続バイトから UTF-8 の 1 文字を復元する。
fn read_utf8_char<R: Read>(first: u8, reader: &mut R) -> io::Result<Option<char>> {
    let width = match first {
        0x00..=0x7f => 1,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => return Ok(None),
    };
    let mut buf = [0u8; 4];
    buf[0] = first;
    for idx in 1..width {
        reader.read_exact(&mut buf[idx..idx + 1])?;
    }
    match std::str::from_utf8(&buf[..width]) {
        Ok(s) => Ok(s.chars().next()),
        Err(_) => Ok(None),
    }
}

#[cfg(unix)]
/// 読み取った制御シーケンスを内部の編集操作へ写像する。
fn interpret_action<R: Read>(first: u8, reader: &mut R) -> io::Result<EditAction> {
    match first {
        b'\n' | b'\r' => Ok(EditAction::Submit),
        0x03 => Ok(EditAction::Interrupt),
        0x04 => Ok(EditAction::Eof),
        0x7f | 0x08 => Ok(EditAction::DeleteLeft),
        0x1b => {
            let mut seq = [0u8; 2];
            if reader.read_exact(&mut seq[..1]).is_err() {
                return Ok(EditAction::Ignore);
            }
            if seq[0] != b'[' {
                return Ok(EditAction::Ignore);
            }
            if reader.read_exact(&mut seq[1..2]).is_err() {
                return Ok(EditAction::Ignore);
            }
            Ok(match seq[1] {
                b'A' => EditAction::HistoryPrev,
                b'B' => EditAction::HistoryNext,
                b'C' => EditAction::MoveRight,
                b'D' => EditAction::MoveLeft,
                _ => EditAction::Ignore,
            })
        }
        _ => {
            if let Some(ch) = read_utf8_char(first, reader)? {
                if ch.is_control() {
                    Ok(EditAction::Ignore)
                } else {
                    Ok(EditAction::InsertChar(ch))
                }
            } else {
                Ok(EditAction::Ignore)
            }
        }
    }
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditAction {
    Submit,
    Interrupt,
    Eof,
    DeleteLeft,
    MoveLeft,
    MoveRight,
    HistoryPrev,
    HistoryNext,
    InsertChar(char),
    Ignore,
}

#[cfg(unix)]
struct EditorSession<'a> {
    buffer: Vec<char>,
    cursor: usize,
    history_index: usize,
    saved_current: Option<Vec<char>>,
    history: &'a History,
}

#[cfg(unix)]
impl<'a> EditorSession<'a> {
    fn new(history: &'a History) -> Self {
        Self {
            buffer: Vec::new(),
            cursor: 0,
            history_index: history.len(),
            saved_current: None,
            history,
        }
    }

    fn buffer(&self) -> &[char] {
        &self.buffer
    }

    fn cursor(&self) -> usize {
        self.cursor
    }

    fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    fn insert_char(&mut self, ch: char) {
        self.buffer.insert(self.cursor, ch);
        self.cursor += 1;
        self.reset_history_cursor();
    }

    fn delete_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        self.buffer.remove(self.cursor);
        self.reset_history_cursor();
        true
    }

    fn move_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        true
    }

    fn move_right(&mut self) -> bool {
        if self.cursor >= self.buffer.len() {
            return false;
        }
        self.cursor += 1;
        true
    }

    fn history_prev(&mut self) -> bool {
        if self.history_index == 0 {
            return false;
        }
        if self.history_index == self.history.len() {
            self.saved_current = Some(self.buffer.clone());
        }
        self.history_index -= 1;
        if let Some(entry) = self.history.get(self.history_index) {
            self.buffer = entry.chars().collect();
            self.cursor = self.buffer.len();
            return true;
        }
        false
    }

    fn history_next(&mut self) -> bool {
        if self.history_index >= self.history.len() {
            return false;
        }
        self.history_index += 1;
        if self.history_index == self.history.len() {
            self.buffer = self.saved_current.clone().unwrap_or_default();
        } else if let Some(entry) = self.history.get(self.history_index) {
            self.buffer = entry.chars().collect();
        }
        self.cursor = self.buffer.len();
        true
    }

    fn into_string(self) -> String {
        self.buffer.into_iter().collect()
    }

    fn reset_history_cursor(&mut self) {
        self.history_index = self.history.len();
        self.saved_current = None;
    }
}

#[cfg(unix)]
/// バッファとカーソル位置に合わせて行全体を再描画する。
fn refresh_line<W: Write>(
    writer: &mut W,
    prompt: &str,
    buffer: &[char],
    cursor: usize,
) -> io::Result<()> {
    let rendered: String = buffer.iter().collect();
    write!(writer, "\r{}{}", prompt, rendered)?;
    write!(writer, "\x1b[K")?;
    let total = prompt.chars().count() + buffer.len();
    let target = prompt.chars().count() + cursor;
    if total > target {
        write!(writer, "\x1b[{}D", total - target)?;
    }
    writer.flush()
}

/// 入力履歴の保持と永続化を司る補助構造体。
struct History {
    entries: Vec<String>,
    path: Option<PathBuf>,
    max_entries: usize,
}

impl History {
    /// 過去の履歴ファイルを読み込み、`History` を初期化する。
    fn load() -> Self {
        let path = history_path();
        let entries = path
            .as_ref()
            .and_then(|p| fs::read_to_string(p).ok())
            .map(|content| content.lines().map(|s| s.to_string()).collect())
            .unwrap_or_default();
        Self {
            entries,
            path,
            max_entries: 1000,
        }
    }

    /// 新しい入力を追加し、空行と直前の重複をスキップする。
    fn add(&mut self, entry: &str) {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return;
        }
        if self.entries.last().map(|s| s.as_str()) == Some(trimmed) {
            return;
        }
        if self.entries.len() == self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(trimmed.to_string());
    }

    /// 登録されている履歴件数を返す。
    fn len(&self) -> usize {
        self.entries.len()
    }

    /// 指定インデックスの履歴エントリを参照する。
    fn get(&self, idx: usize) -> Option<&str> {
        self.entries.get(idx).map(|s| s.as_str())
    }

    /// 現在の履歴をファイルへ書き出す。
    fn save(&self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(path)?;
        for entry in &self.entries {
            writeln!(file, "{}", entry)?;
        }
        Ok(())
    }
}

/// 履歴ファイルの保存場所を環境変数とユーザーのホームから決定する。
fn history_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os("TYPELANG_HISTORY_FILE") {
        return Some(PathBuf::from(path));
    }
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .map(|home| home.join(".typelang_repl_history"))
}

#[cfg(unix)]
/// Raw モードへの切り替えと復帰を担う RAII ガード。
struct RawMode {
    original: Termios,
}

#[cfg(unix)]
impl RawMode {
    /// 標準入力の termios 設定を Raw モードへ変更する。
    #[allow(unexpected_cfgs)]
    #[cfg_attr(coverage, coverage(off))]
    fn new() -> io::Result<Self> {
        let fd = 0; // 標準入力のファイルディスクリプタ
        let mut termios = Termios::default();
        if unsafe { tcgetattr(fd, &mut termios as *mut _) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut raw = termios;
        // OS ごとの差分は `cfmakeraw` に任せて Raw モードへ移行する。
        unsafe {
            cfmakeraw(&mut raw as *mut _);
        }
        if unsafe { tcsetattr(fd, TCSANOW, &raw as *const _) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { original: termios })
    }
}

#[cfg(unix)]
impl Drop for RawMode {
    /// スコープ終了時に取得済みの termios 設定へ戻す。
    #[allow(unexpected_cfgs)]
    #[cfg_attr(coverage, coverage(off))]
    fn drop(&mut self) {
        let fd = 0;
        unsafe {
            let _ = tcsetattr(fd, TCSANOW, &self.original as *const _);
        }
    }
}

#[cfg(unix)]
const TCSANOW: i32 = 0;

#[cfg(unix)]
#[repr(C)]
#[derive(Clone, Copy)]
/// POSIX 端末属性 (`termios`) を Rust 表現に写した構造体。
struct Termios {
    c_iflag: u32,
    c_oflag: u32,
    c_cflag: u32,
    c_lflag: u32,
    c_line: u8,
    c_cc: [u8; NCCS],
    c_ispeed: u32,
    c_ospeed: u32,
}

#[cfg(unix)]
impl Default for Termios {
    /// ゼロ初期化された `Termios` を構築する。
    fn default() -> Self {
        Self {
            c_iflag: 0,
            c_oflag: 0,
            c_cflag: 0,
            c_lflag: 0,
            c_line: 0,
            c_cc: [0; NCCS],
            c_ispeed: 0,
            c_ospeed: 0,
        }
    }
}

#[cfg(unix)]
#[cfg(any(target_os = "linux", target_os = "android"))]
const NCCS: usize = 32;
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
))]
const NCCS: usize = 20;
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
)))]
const NCCS: usize = 32;

#[cfg(unix)]
extern "C" {
    /// libc の `tcgetattr` を直接呼び出す。
    fn tcgetattr(fd: i32, termios: *mut Termios) -> i32;
    /// libc の `tcsetattr` を直接呼び出す。
    fn tcsetattr(fd: i32, optional_actions: i32, termios: *const Termios) -> i32;
    /// libc の `cfmakeraw` を実行する。
    fn cfmakeraw(termios: *mut Termios);
}

#[cfg(test)]
mod tests {
    use super::{history_path, read_utf8_char, History};
    use std::env;
    use std::fs;
    use std::io::Cursor;
    use std::sync::{Mutex, OnceLock};

    #[test]
    /// 同じ入力が連続しても履歴に重複登録されないことを確かめる。
    fn history_add_deduplicates() {
        let mut history = History {
            entries: Vec::new(),
            path: None,
            max_entries: 5,
        };
        history.add("foo");
        history.add("foo");
        history.add("bar");
        assert_eq!(history.entries, vec!["foo", "bar"]);
    }

    #[test]
    /// 空行や末尾空白が履歴から取り除かれることを検証する。
    fn history_add_skips_empty_and_trims() {
        let mut history = History {
            entries: Vec::new(),
            path: None,
            max_entries: 3,
        };
        history.add("   ");
        history.add(" foo ");
        history.add("foo");
        history.add("bar");
        assert_eq!(history.entries, vec!["foo", "bar"]);
    }

    #[test]
    /// 上限を超えた履歴が先頭から削除されることを確認する。
    fn history_respects_max_entries() {
        let mut history = History {
            entries: vec!["0".into(), "1".into(), "2".into()],
            path: None,
            max_entries: 3,
        };
        history.add("3");
        assert_eq!(history.entries, vec!["1", "2", "3"]);
    }

    /// 環境変数を書き換えるテストを直列化するためのヘルパ。
    fn with_env_lock<T>(f: impl FnOnce() -> T) -> T {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        let lock = GUARD.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().unwrap();
        f()
    }

    #[cfg_attr(miri, ignore = "Miri isolation blocks directory creation APIs")]
    #[test]
    /// 履歴が保存・再読込で失われないことを検証する。
    fn history_save_and_load_roundtrip() {
        with_env_lock(|| {
            let dir = env::temp_dir().join("typelang_history_tests");
            fs::create_dir_all(&dir).unwrap();
            let path = dir.join(format!(
                "history_{}.txt",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));

            let history = History {
                entries: vec!["foo".into(), "bar".into()],
                path: Some(path.clone()),
                max_entries: 10,
            };
            history.save().unwrap();

            env::set_var("TYPELANG_HISTORY_FILE", &path);
            let loaded = History::load();
            env::remove_var("TYPELANG_HISTORY_FILE");

            assert_eq!(loaded.entries, vec!["foo", "bar"]);
            assert_eq!(loaded.path.as_ref(), Some(&path));

            fs::remove_file(path).unwrap();
        });
    }

    #[test]
    /// 環境変数による履歴パス指定が優先されることを確認する。
    fn history_path_prefers_env_variable() {
        with_env_lock(|| {
            let dir = env::temp_dir();
            let path = dir.join("typelang_history_env_test.txt");
            env::set_var("TYPELANG_HISTORY_FILE", &path);
            let resolved = history_path().unwrap();
            assert!(resolved.ends_with("typelang_history_env_test.txt"));
            env::remove_var("TYPELANG_HISTORY_FILE");
        });
    }

    #[test]
    /// 複数バイトの UTF-8 文字が正しく復元されるか検証する。
    fn read_utf8_char_handles_multibyte() {
        let mut cursor = Cursor::new(vec![0x81, 0x82]);
        let ch = read_utf8_char(0xe3, &mut cursor).unwrap().unwrap();
        assert_eq!(ch, 'あ');
    }

    #[cfg(unix)]
    #[test]
    /// 再描画後のカーソル位置が期待通り手前へ戻るか確認する。
    fn refresh_line_repositions_cursor() {
        let mut buffer: Vec<u8> = Vec::new();
        super::refresh_line(&mut buffer, ":: ", &['a', 'b', 'c'], 1).unwrap();
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains(":: abc"));
        assert!(output.contains("\x1b[K"));
        assert!(output.contains("\x1b[2D"));
    }

    #[cfg(unix)]
    #[test]
    /// エスケープシーケンスが履歴操作やカーソル移動のアクションへ変換されるかを検証する。
    fn interpret_action_and_session_navigation() {
        use super::{interpret_action, EditAction, EditorSession};

        let history = History {
            entries: vec!["first".into(), "second".into()],
            path: None,
            max_entries: 10,
        };
        let mut session = EditorSession::new(&history);
        for ch in "tmp".chars() {
            session.insert_char(ch);
        }

        // 上矢印で過去の履歴を辿る。
        let mut reader = Cursor::new(vec![b'[', b'A']);
        let action = interpret_action(0x1b, &mut reader).unwrap();
        assert_eq!(action, EditAction::HistoryPrev);
        assert!(session.history_prev());
        assert_eq!(session.buffer().iter().collect::<String>(), "second");

        // 下矢印で保存済みの入力へ戻る。
        let mut reader = Cursor::new(vec![b'[', b'B']);
        let action = interpret_action(0x1b, &mut reader).unwrap();
        assert_eq!(action, EditAction::HistoryNext);
        assert!(session.history_next());
        assert_eq!(session.buffer().iter().collect::<String>(), "tmp");

        // 左矢印でカーソルを 1 文字戻す。
        let mut reader = Cursor::new(vec![b'[', b'D']);
        let action = interpret_action(0x1b, &mut reader).unwrap();
        assert_eq!(action, EditAction::MoveLeft);
        assert!(session.move_left());
        assert_eq!(session.cursor(), session.buffer().len() - 1);
    }

    #[cfg(unix)]
    #[test]
    /// カーソル境界や履歴遷移の失敗分岐を含めてセッション操作を網羅する。
    fn editor_session_covers_boundary_branches() {
        use super::EditorSession;

        let history = History {
            entries: vec!["first".into(), "second".into()],
            path: None,
            max_entries: 10,
        };
        let mut session = EditorSession::new(&history);
        assert!(session.is_empty());
        assert!(!session.delete_left());
        assert!(!session.move_left());

        session.insert_char('a');
        session.insert_char('b');
        assert!(session.move_left());
        assert!(session.delete_left());
        assert!(session.move_right());
        assert!(!session.move_right());
        assert!(session.move_left());
        assert!(!session.move_left());

        assert!(session.history_prev());
        assert!(session.history_prev());
        assert!(!session.history_prev());
        assert!(session.history_next());
        assert!(session.history_next());
        assert!(!session.history_next());

        session.insert_char('z');
        assert!(!session.history_next());
    }

    #[cfg(unix)]
    #[test]
    /// 不完全なエスケープシーケンスや非表示文字が無視されるか検証する。
    fn interpret_action_ignores_incomplete_sequences() {
        use super::{interpret_action, EditAction};

        let mut reader = Cursor::new(Vec::<u8>::new());
        assert!(matches!(
            interpret_action(0x1b, &mut reader).unwrap(),
            EditAction::Ignore
        ));

        let mut reader = Cursor::new(vec![b'X']);
        assert!(matches!(
            interpret_action(0x1b, &mut reader).unwrap(),
            EditAction::Ignore
        ));

        let mut reader = Cursor::new(vec![b'[']);
        assert!(matches!(
            interpret_action(0x1b, &mut reader).unwrap(),
            EditAction::Ignore
        ));

        let mut reader = Cursor::new(vec![b'[', b'Z']);
        assert!(matches!(
            interpret_action(0x1b, &mut reader).unwrap(),
            EditAction::Ignore
        ));

        let mut reader = Cursor::new(Vec::<u8>::new());
        assert!(matches!(
            interpret_action(0x01, &mut reader).unwrap(),
            EditAction::Ignore
        ));
    }

    #[test]
    /// 無効な UTF-8 先頭バイトが None を返すか確認する。
    fn read_utf8_char_rejects_invalid_lead() {
        let mut cursor = Cursor::new(vec![0xff, 0x00, 0x00]);
        let ch = read_utf8_char(0x80, &mut cursor).unwrap();
        assert!(ch.is_none());
    }
}
