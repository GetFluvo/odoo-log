//! Chipmunk parser plugin for Odoo logs.
//!
//! Every input line becomes one row. Continuation lines (tracebacks, view errors)
//! inherit level, database and logger from the header above them, so a filter on
//! `ERROR` also shows the traceback underneath.

use odoo_log::{Header, Line, classify};
use plugins_api::{
    parser::{
        ColumnInfo, ColumnsRenderOptions, ParseError, ParseReturn, ParseYield, ParsedMessage,
        Parser, ParserConfig, RenderOptions,
    },
    parser_export,
    shared_types::{ConfigItem, ConfigSchemaItem, InitError, Version},
};

/// Owned copy of the fields a continuation line inherits.
#[derive(Default)]
struct Owner {
    level: String,
    db: String,
    logger: String,
}

impl From<&Header<'_>> for Owner {
    fn from(h: &Header<'_>) -> Self {
        Owner {
            level: h.level.as_str().to_owned(),
            db: h.db.unwrap_or("?").to_owned(),
            logger: h.logger.to_owned(),
        }
    }
}

#[derive(Default)]
pub struct OdooParser {
    owner: Option<Owner>,
}

impl OdooParser {
    fn row(&mut self, raw: &str) -> Vec<String> {
        // `classify` needs the previous header only to decide header vs continuation;
        // inheritance is done here from the owned copy so state survives across calls.
        let mut none = None;
        match classify(&mut none, raw) {
            Line::Header(h) => {
                self.owner = Some(Owner::from(&h));
                vec![
                    h.timestamp.to_owned(),
                    h.pid.to_string(),
                    h.level.as_str().to_owned(),
                    h.db.unwrap_or("?").to_owned(),
                    h.logger.to_owned(),
                    h.message.to_owned(),
                ]
            }
            Line::Orphan(text) | Line::Continuation { text, .. } => {
                let o = self.owner.as_ref();
                vec![
                    String::new(),
                    String::new(),
                    o.map_or_else(String::new, |o| o.level.clone()),
                    o.map_or_else(String::new, |o| o.db.clone()),
                    o.map_or_else(String::new, |o| o.logger.clone()),
                    text.to_owned(),
                ]
            }
        }
    }
}

impl Parser for OdooParser {
    fn get_version() -> Version {
        Version::new(0, 1, 0)
    }

    fn get_config_schemas() -> Vec<ConfigSchemaItem> {
        Vec::new()
    }

    fn get_render_options() -> RenderOptions {
        let cols = vec![
            ColumnInfo::new("Datetime", "Timestamp as written by Odoo", 170),
            ColumnInfo::new("PID", "Process id", 50),
            ColumnInfo::new("Level", "Log level (inherited on traceback lines)", 70),
            ColumnInfo::new("DB", "Database, ? when none is bound", 120),
            ColumnInfo::new("Logger", "Python logger name", 220),
            ColumnInfo::new("Message", "Message or continuation line", -1),
        ];
        RenderOptions::new(Some(ColumnsRenderOptions::new(cols, 30, 600)))
    }

    fn create(
        _general_configs: ParserConfig,
        _plugins_configs: Vec<ConfigItem>,
    ) -> Result<Self, InitError> {
        Ok(Self::default())
    }

    fn parse(
        &mut self,
        data: &[u8],
        _timestamp: Option<u64>,
    ) -> Result<impl Iterator<Item = ParseReturn>, ParseError> {
        if data.is_empty() {
            return Err(ParseError::Eof);
        }
        let mut out = Vec::new();
        let mut rest = data;
        while !rest.is_empty() {
            // A final line without '\n' is taken as-is, like Chipmunk's string parser.
            let (line, consumed) = match rest.iter().position(|&b| b == b'\n') {
                Some(i) => (&rest[..i], i + 1),
                None => (rest, rest.len()),
            };
            let text = String::from_utf8_lossy(line);
            let cols = self.row(&text);
            out.push(ParseReturn::new(
                consumed as u64,
                Some(ParseYield::Message(ParsedMessage::Columns(cols))),
            ));
            rest = &rest[consumed..];
        }
        Ok(out.into_iter())
    }
}

parser_export!(OdooParser);
