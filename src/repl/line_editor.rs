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

pub struct LineEditor {
    history: History,
}

impl LineEditor {
    pub fn new() -> Self {
        Self {
            history: History::load(),
        }
    }

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

    pub fn add_history(&mut self, entry: &str) {
        self.history.add(entry);
    }

    pub fn save_history(&self) -> io::Result<()> {
        self.history.save()
    }
}

impl Default for LineEditor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(unix))]
impl LineEditor {
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
struct EscapeContext<'a> {
    history: &'a History,
    history_index: &'a mut usize,
    saved_current: &'a mut Option<Vec<char>>,
}

#[cfg(unix)]
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

struct History {
    entries: Vec<String>,
    path: Option<PathBuf>,
    max_entries: usize,
}

impl History {
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

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn get(&self, idx: usize) -> Option<&str> {
        self.entries.get(idx).map(|s| s.as_str())
    }

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
struct RawMode {
    original: Termios,
}

#[cfg(unix)]
impl RawMode {
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
    fn tcgetattr(fd: i32, termios: *mut Termios) -> i32;
    fn tcsetattr(fd: i32, optional_actions: i32, termios: *const Termios) -> i32;
    fn cfmakeraw(termios: *mut Termios);
}

#[cfg(test)]
mod tests {
    use super::History;

    #[test]
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
}
