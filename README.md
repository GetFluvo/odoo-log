# odoo-log

Parse Odoo server and build logs.

| Crate | What |
|---|---|
| `crates/odoo-log` | Zero-copy parser: header lines (timestamp, pid, level, db, logger, message), continuation lines (tracebacks) attached to their header, grouping into records. No dependencies. |
| `crates/chipmunk-odoo` | [Chipmunk](https://github.com/esrlabs/chipmunk) parser plugin (WASM component) built on `odoo-log`. |

## Chipmunk plugin

```sh
cd crates/chipmunk-odoo
cargo build --release --target wasm32-wasip2
P=~/.chipmunk/plugins/parsers/chipmunk-odoo
mkdir -p $P
cp target/wasm32-wasip2/release/chipmunk_odoo.wasm $P/chipmunk-odoo.wasm
cp chipmunk-odoo.toml $P/
```

Then restart Chipmunk and open a log via **Open File(s) with Plugin...** (or
Ctrl+Shift+P) and pick the **Odoo log** parser. A plain open uses the text parser.
Traceback lines inherit the level of the log call above them, so filtering on
`ERROR`/`CRITICAL` keeps the traceback visible.

Do not use the Plugin Manager's *Add* button on the installed directory itself: Chipmunk
clears the destination before copying, which deletes the plugin. Add from a separate
directory instead.

### Colours

Parser plugins cannot set colours; Chipmunk colours rows with filters. Import
`crates/chipmunk-odoo/presets/odoo.json` in the **Presets** panel and enable the **Odoo**
preset (CRITICAL, ERROR, Odoo exceptions, WARNING, migrations, module loading, loaded).

## Test fixtures

Fixtures are real logs with client names, databases and paths replaced. Never commit an
unredacted log.
