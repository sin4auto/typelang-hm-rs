// パス: src/repl/line_editor.rs
// 役割: Terminal line editor handling history and cursor movement
// 意図: Provide portable interactive input for the REPL
// 関連ファイル: src/repl/cmd.rs, src/repl/printer.rs, src/repl/util.rs
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

/// 行入力の結果種別。
pub enum ReadResult {
    Line(String),
    Eof,
    Interrupted,
}

/// 履歴管理付きの行入力処理を担う構造体。
pub struct LineEditor {
    history: History,
}

/// LineEditor の操作メソッド。
impl LineEditor {
    /// 履歴を読み込んで新しいエディタを生成する。
    pub fn new() -> Self {
        Self {
            history: History::load(),
        }
    }

    /// プロンプト付きで1行入力を読み取る。
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

    /// 入力行を履歴へ追加する。
    pub fn add_history(&mut self, entry: &str) {
        self.history.add(entry);
    }

    /// 履歴を永続化する。
    pub fn save_history(&self) -> io::Result<()> {
        self.history.save()
    }
}

/// 既定値として `new` を利用する。
impl Default for LineEditor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(unix))]
impl LineEditor {
    /// UNIX以外の環境向けに行入力を読む。
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
    /// UNIX環境で端末をRawモードにして読み取る。
    fn read_line_unix(&mut self, prompt: &str) -> io::Result<ReadResult> {
        let _raw = RawMode::new()?;
        let mut stdout = io::stdout();
        write!(stdout, "{}", prompt)?;
        stdout.flush()?;

        let mut buffer: Vec<char> = Vec::new();
        let mut cursor: usize = 0;
        let mut history_index = self.history.len();
        let mut saved_current: Option<Vec<char>> = None;

        let stdin = io::stdin();
        let mut stdin = stdin.lock();
        loop {
            let mut byte = [0u8; 1];
            if stdin.read(&mut byte)? == 0 {
                return Ok(ReadResult::Eof);
            }
            let b = byte[0];
            match b {
                b'\n' | b'\r' => {
                    write!(stdout, "\r\n")?;
                    stdout.flush()?;
                    let line: String = buffer.into_iter().collect();
                    return Ok(ReadResult::Line(line));
                }
                0x03 => {
                    write!(stdout, "^C\r\n")?;
                    stdout.flush()?;
                    return Ok(ReadResult::Interrupted);
                }
                0x04 => {
                    if buffer.is_empty() {
                        return Ok(ReadResult::Eof);
                    }
                }
                0x7f | 0x08 => {
                    if cursor > 0 {
                        cursor -= 1;
                        buffer.remove(cursor);
                        history_index = self.history.len();
                        saved_current = None;
                        refresh_line(&mut stdout, prompt, &buffer, cursor)?;
                    }
                }
                0x1b => {
                    let context = EscapeContext {
                        history: &self.history,
                        history_index: &mut history_index,
                        saved_current: &mut saved_current,
                    };
                    handle_escape(
                        &mut stdin,
                        &mut stdout,
                        prompt,
                        &mut buffer,
                        &mut cursor,
                        context,
                    )?;
                }
                _ => {
                    if let Some(ch) = read_utf8_char(b, &mut stdin)? {
                        if ch.is_control() {
                            continue;
                        }
                        buffer.insert(cursor, ch);
                        cursor += 1;
                        history_index = self.history.len();
                        saved_current = None;
                        refresh_line(&mut stdout, prompt, &buffer, cursor)?;
                    }
                }
            }
        }
    }
}

/// UTF-8の先頭バイト列から文字を復元する。
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
/// エスケープシーケンス処理に必要な状態をまとめる。
struct EscapeContext<'a> {
    history: &'a History,
    history_index: &'a mut usize,
    saved_current: &'a mut Option<Vec<char>>,
}

#[cfg(unix)]
/// 端末から届いたエスケープシーケンスを処理する。
fn handle_escape<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    prompt: &str,
    buffer: &mut Vec<char>,
    cursor: &mut usize,
    context: EscapeContext<'_>,
) -> io::Result<()> {
    let EscapeContext {
        history,
        history_index,
        saved_current,
    } = context;
    let mut seq = [0u8; 2];
    if reader.read_exact(&mut seq[..1]).is_err() {
        return Ok(());
    }
    if seq[0] != b'[' {
        return Ok(());
    }
    if reader.read_exact(&mut seq[1..2]).is_err() {
        return Ok(());
    }
    match seq[1] {
        b'A' => {
            if *history_index > 0 {
                if *history_index == history.len() {
                    *saved_current = Some(buffer.clone());
                }
                *history_index -= 1;
                if let Some(entry) = history.get(*history_index) {
                    *buffer = entry.chars().collect();
                    *cursor = buffer.len();
                    refresh_line(writer, prompt, buffer, *cursor)?;
                }
            }
        }
        b'B' => {
            if *history_index < history.len() {
                *history_index += 1;
                let restored = if *history_index == history.len() {
                    saved_current.clone().unwrap_or_default()
                } else {
                    history.get(*history_index).unwrap().chars().collect()
                };
                *buffer = restored;
                *cursor = buffer.len();
                refresh_line(writer, prompt, buffer, *cursor)?;
            }
        }
        b'C' => {
            if *cursor < buffer.len() {
                *cursor += 1;
                write!(writer, "\x1b[C")?;
                writer.flush()?;
            }
        }
        b'D' => {
            if *cursor > 0 {
                *cursor -= 1;
                write!(writer, "\x1b[D")?;
                writer.flush()?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(unix)]
/// 1行分の表示を再描画する。
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

/// 入力履歴を保持・永続化する構造体。
struct History {
    entries: Vec<String>,
    path: Option<PathBuf>,
    max_entries: usize,
}

impl History {
    /// 保存済みの履歴を読み込む。
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

    /// 履歴に項目を追加する。
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

    /// 登録済みの履歴数を返す。
    fn len(&self) -> usize {
        self.entries.len()
    }

    /// 履歴の特定位置を参照する。
    fn get(&self, idx: usize) -> Option<&str> {
        self.entries.get(idx).map(|s| s.as_str())
    }

    /// 履歴をファイルに保存する。
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

/// 履歴ファイルの探索および環境変数の優先順位を決める。
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
/// 端末設定をRawモードへ切り替えるためのガード。
struct RawMode {
    original: Termios,
}

#[cfg(unix)]
impl RawMode {
    /// 標準入力をRawモードへ切り替える。
    fn new() -> io::Result<Self> {
        let fd = 0; // stdin
        let mut termios = Termios::default();
        if unsafe { tcgetattr(fd, &mut termios as *mut _) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut raw = termios;
        // `cfmakeraw` に任せることで OS ごとの差分を吸収して Raw モードへ移行する。
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
    /// スコープ終了時に元の端末設定へ戻す。
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
/// 端末属性を保持するための構造体。
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
    /// すべてのフィールドをゼロ初期化する。
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
    /// POSIX の `tcgetattr` を呼び出す。
    fn tcgetattr(fd: i32, termios: *mut Termios) -> i32;
    /// POSIX の `tcsetattr` を呼び出す。
    fn tcsetattr(fd: i32, optional_actions: i32, termios: *const Termios) -> i32;
    /// POSIX の `cfmakeraw` を呼び出す。
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
    /// 重複した履歴が追加されないことを確認する。
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
    /// 空入力や末尾空白が除外されることを確認する。
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
    /// 履歴サイズが上限を超えないことを確認する。
    fn history_respects_max_entries() {
        let mut history = History {
            entries: vec!["0".into(), "1".into(), "2".into()],
            path: None,
            max_entries: 3,
        };
        history.add("3");
        assert_eq!(history.entries, vec!["1", "2", "3"]);
    }

    /// テストで環境変数を直列化するヘルパ。
    fn with_env_lock<T>(f: impl FnOnce() -> T) -> T {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        let lock = GUARD.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().unwrap();
        f()
    }

    #[cfg_attr(miri, ignore = "Miri isolation blocks directory creation APIs")]
    #[test]
    /// 履歴の保存と再読込が正しく行われるか検証する。
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
    /// 履歴ファイルパスが環境変数を優先することを確認する。
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
    /// UTF-8の複数バイト文字を読めることを検証する。
    fn read_utf8_char_handles_multibyte() {
        let mut cursor = Cursor::new(vec![0x81, 0x82]);
        let ch = read_utf8_char(0xe3, &mut cursor).unwrap().unwrap();
        assert_eq!(ch, 'あ');
    }

    #[cfg(unix)]
    #[test]
    /// 行再描画時にカーソルが戻ることを確認する。
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
    /// エスケープシーケンスで履歴とカーソルが移動することを検証する。
    fn handle_escape_navigates_history_and_cursor() {
        use super::{handle_escape, EscapeContext};

        let history = History {
            entries: vec!["first".into(), "second".into()],
            path: None,
            max_entries: 10,
        };
        let mut history_index = history.len();
        let mut saved_current = None;
        let mut buffer: Vec<char> = "tmp".chars().collect();
        let mut cursor = buffer.len();

        // Up arrow: recall last history entry
        let mut reader = Cursor::new(vec![b'[', b'A']);
        let mut writer: Vec<u8> = Vec::new();
        handle_escape(
            &mut reader,
            &mut writer,
            "> ",
            &mut buffer,
            &mut cursor,
            EscapeContext {
                history: &history,
                history_index: &mut history_index,
                saved_current: &mut saved_current,
            },
        )
        .unwrap();
        assert_eq!(buffer.iter().collect::<String>(), "second");
        assert_eq!(cursor, buffer.len());
        assert!(saved_current.is_some());

        // Down arrow: restore saved current input
        let mut reader = Cursor::new(vec![b'[', b'B']);
        handle_escape(
            &mut reader,
            &mut writer,
            "> ",
            &mut buffer,
            &mut cursor,
            EscapeContext {
                history: &history,
                history_index: &mut history_index,
                saved_current: &mut saved_current,
            },
        )
        .unwrap();
        assert_eq!(buffer.iter().collect::<String>(), "tmp");
        assert_eq!(cursor, buffer.len());

        // Left arrow: move cursor left when possible
        buffer = "hi".chars().collect();
        cursor = buffer.len();
        let mut reader = Cursor::new(vec![b'[', b'D']);
        handle_escape(
            &mut reader,
            &mut writer,
            "> ",
            &mut buffer,
            &mut cursor,
            EscapeContext {
                history: &history,
                history_index: &mut history_index,
                saved_current: &mut saved_current,
            },
        )
        .unwrap();
        assert_eq!(cursor, buffer.len() - 1);
        assert!(String::from_utf8(writer.clone())
            .unwrap()
            .contains("\x1b[D"));
    }
}
