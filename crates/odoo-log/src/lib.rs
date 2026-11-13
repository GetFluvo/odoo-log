//! Parser for Odoo server logs.
//!
//! Odoo writes one header line per log call:
//!
//! ```text
//! 2026-11-13 15:45:46,700 2056 ERROR acme-main odoo.registry: Failed to load registry
//! ```
//!
//! followed by zero or more continuation lines (tracebacks, view errors, pip output).
//! This crate offers two levels:
//!
//! * [`parse_header`] / [`Lines`]: line by line, every continuation line carries the
//!   header it belongs to. Streaming friendly; used by the Chipmunk plugin.
//! * [`records`]: groups a header with its continuation lines into one [`Record`].

use std::fmt;

/// Odoo log level. Odoo uses the stdlib levels; anything else is kept verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level<'a> {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
    Other(&'a str),
}

impl<'a> Level<'a> {
    fn parse(s: &'a str) -> Self {
        match s {
            "DEBUG" => Level::Debug,
            "INFO" => Level::Info,
            "WARNING" => Level::Warning,
            "ERROR" => Level::Error,
            "CRITICAL" => Level::Critical,
            other => Level::Other(other),
        }
    }

    pub fn as_str(&self) -> &'a str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warning => "WARNING",
            Level::Error => "ERROR",
            Level::Critical => "CRITICAL",
            Level::Other(s) => s,
        }
    }

    /// True for ERROR and CRITICAL.
    pub fn is_failure(&self) -> bool {
        matches!(self, Level::Error | Level::Critical)
    }
}

impl fmt::Display for Level<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The parsed first line of a log call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header<'a> {
    /// `YYYY-MM-DD HH:MM:SS,mmm`, server local time as written by Odoo.
    pub timestamp: &'a str,
    pub pid: u32,
    pub level: Level<'a>,
    /// `None` when Odoo logged `?` (no database bound yet).
    pub db: Option<&'a str>,
    pub logger: &'a str,
    pub message: &'a str,
}

fn is_timestamp(s: &[u8]) -> bool {
    // YYYY-MM-DD HH:MM:SS,mmm
    const SHAPE: &[u8; 23] = b"dddd-dd-dd dd:dd:dd,ddd";
    s.len() == 23
        && s.iter().zip(SHAPE).all(|(c, p)| match p {
            b'd' => c.is_ascii_digit(),
            p => c == p,
        })
}

/// Parse a header line. Returns `None` for continuation lines.
///
/// A trailing `\r`, trailing whitespace and ANSI colour codes around the level
/// (Odoo colours the level when logging to a TTY) are tolerated.
pub fn parse_header(line: &str) -> Option<Header<'_>> {
    let line = line.trim_end();
    let ts = line.get(..23)?;
    if !is_timestamp(ts.as_bytes()) {
        return None;
    }
    let rest = line.get(23..)?.strip_prefix(' ')?;

    let (pid, rest) = rest.split_once(' ')?;
    let pid: u32 = pid.parse().ok()?;

    let (level, rest) = rest.split_once(' ')?;
    let level = strip_ansi(level);
    if level.is_empty() || !level.bytes().all(|b| b.is_ascii_uppercase()) {
        return None;
    }

    let (db, rest) = rest.split_once(' ')?;
    let db = (db != "?").then_some(db);

    // The logger never contains ": ", the message may.
    let (logger, message) = match rest.split_once(": ") {
        Some((l, m)) => (l, m),
        None => (rest.strip_suffix(':').unwrap_or(rest), ""),
    };

    Some(Header {
        timestamp: ts,
        pid,
        level: Level::parse(level),
        db,
        logger,
        message,
    })
}

/// Strip `ESC[...m` sequences without allocating. Odoo wraps the level as
/// `ESC[1;32mESC[1;49mINFOESC[0m`, so the text is the run between the last `m` of
/// the leading codes and the next `ESC`.
fn strip_ansi(s: &str) -> &str {
    if !s.contains('\x1b') {
        return s;
    }
    let mut start = 0;
    while s[start..].starts_with('\x1b') {
        match s[start..].find('m') {
            Some(i) => start += i + 1,
            None => return s,
        }
    }
    let end = s[start..].find('\x1b').map_or(s.len(), |i| start + i);
    &s[start..end]
}

/// One line of a log, classified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line<'a> {
    /// A header line.
    Header(Header<'a>),
    /// A continuation of `owner` (traceback, multi-line message).
    Continuation { owner: Header<'a>, text: &'a str },
    /// Text before the first header (e.g. pip / build tool output).
    Orphan(&'a str),
}

impl<'a> Line<'a> {
    /// The header this line belongs to, if any.
    pub fn owner(&self) -> Option<&Header<'a>> {
        match self {
            Line::Header(h) | Line::Continuation { owner: h, .. } => Some(h),
            Line::Orphan(_) => None,
        }
    }
}

/// Iterator over classified lines. Keeps the last header so continuation lines
/// inherit its level/db/logger.
pub struct Lines<'a> {
    inner: std::str::Lines<'a>,
    last: Option<Header<'a>>,
}

impl<'a> Iterator for Lines<'a> {
    type Item = Line<'a>;

    fn next(&mut self) -> Option<Line<'a>> {
        let raw = self.inner.next()?;
        Some(classify(&mut self.last, raw))
    }
}

/// Classify one raw line given the previous header. Exposed for streaming callers
/// that do their own line splitting (see the Chipmunk plugin).
pub fn classify<'a>(last: &mut Option<Header<'a>>, raw: &'a str) -> Line<'a> {
    if let Some(h) = parse_header(raw) {
        *last = Some(h);
        Line::Header(h)
    } else {
        let text = raw.strip_suffix('\r').unwrap_or(raw);
        match last {
            Some(owner) => Line::Continuation {
                owner: *owner,
                text,
            },
            None => Line::Orphan(text),
        }
    }
}

/// Classify every line of `text`.
pub fn lines(text: &str) -> Lines<'_> {
    Lines {
        inner: text.lines(),
        last: None,
    }
}

/// A header with its continuation lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record<'a> {
    /// `None` for orphan text before the first header.
    pub header: Option<Header<'a>>,
    /// Continuation lines (or the orphan lines), without line endings.
    pub body: Vec<&'a str>,
}

impl Record<'_> {
    /// True if the body looks like a Python traceback.
    pub fn has_traceback(&self) -> bool {
        self.body
            .iter()
            .any(|l| l.starts_with("Traceback (most recent call last):"))
    }
}

/// Group `text` into records.
pub fn records(text: &str) -> Vec<Record<'_>> {
    let mut out: Vec<Record<'_>> = Vec::new();
    for line in lines(text) {
        match line {
            Line::Header(h) => out.push(Record {
                header: Some(h),
                body: Vec::new(),
            }),
            Line::Continuation { text, .. } => out
                .last_mut()
                .expect("continuation always follows a header")
                .body
                .push(text),
            Line::Orphan(text) => match out.last_mut() {
                Some(r) if r.header.is_none() => r.body.push(text),
                _ => out.push(Record {
                    header: None,
                    body: vec![text],
                }),
            },
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_with_db() {
        let h = parse_header(
            "2026-11-13 15:45:46,700 2056 ERROR acme-main odoo.registry: Failed to load registry ",
        )
        .unwrap();
        assert_eq!(h.timestamp, "2026-11-13 15:45:46,700");
        assert_eq!(h.pid, 2056);
        assert_eq!(h.level, Level::Error);
        assert_eq!(h.db, Some("acme-main"));
        assert_eq!(h.logger, "odoo.registry");
        assert_eq!(h.message, "Failed to load registry");
    }

    #[test]
    fn header_without_db_and_colon_in_message() {
        let h = parse_header(
            "2026-11-13 15:45:21,083 2056 WARNING ? odoo.tools.config: option --without-demo: since 19.0, invalid",
        )
        .unwrap();
        assert_eq!(h.db, None);
        assert_eq!(h.logger, "odoo.tools.config");
        assert_eq!(h.message, "option --without-demo: since 19.0, invalid");
    }

    #[test]
    fn ansi_coloured_level() {
        let h =
            parse_header("2026-11-13 15:45:21,083 7 \x1b[1;32m\x1b[1;49mINFO\x1b[0m db odoo: hi")
                .unwrap();
        assert_eq!(h.level, Level::Info);
    }

    #[test]
    fn non_headers() {
        assert!(parse_header("Traceback (most recent call last):").is_none());
        assert!(parse_header("  File \"x.py\", line 1, in y").is_none());
        assert!(parse_header("").is_none());
        assert!(parse_header("2026-11-13 15:45:21 7 INFO db odoo: no millis").is_none());
    }

    #[test]
    fn orphans_before_first_header() {
        let r = records("Collecting foo\nDone\n2026-11-13 15:45:21,083 1 INFO ? odoo: x\n  more\n");
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].header, None);
        assert_eq!(r[0].body, ["Collecting foo", "Done"]);
        assert_eq!(r[1].body, ["  more"]);
    }
}
