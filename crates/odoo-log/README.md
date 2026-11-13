# odoo-log

Zero-copy parser for Odoo server and build logs, no dependencies.

```rust
use odoo_log::{records, Level};

let log = "2026-11-13 15:45:46,700 2056 ERROR acme odoo.registry: Failed to load registry\n\
           Traceback (most recent call last):\n  File \"x.py\", line 1, in y\n";

for r in records(log) {
    let h = r.header.unwrap();
    assert_eq!(h.level, Level::Error);
    assert_eq!(h.db, Some("acme"));
    assert!(r.has_traceback());
}
```

- `parse_header(line)`: timestamp, pid, level, database (`None` for `?`), logger, message.
  Tolerates ANSI colours around the level and trailing whitespace.
- `lines(text)` / `classify`: line-by-line, continuation lines carry their owning header
  (streaming friendly).
- `records(text)`: header plus continuation lines (tracebacks) as one record.

Part of [GetFluvo/odoo-log](https://github.com/GetFluvo/odoo-log), which also ships a
[Chipmunk](https://github.com/esrlabs/chipmunk) log viewer plugin.

License: MIT
