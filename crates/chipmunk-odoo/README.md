# Odoo log

Parses Odoo server and build logs into columns: Datetime, PID, Level, DB, Logger, Message.
Traceback lines inherit the level, database and logger of the log call above them, so a
search or filter on `ERROR` / `CRITICAL` keeps the traceback visible.
