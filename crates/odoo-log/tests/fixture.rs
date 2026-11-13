use odoo_log::{Level, Line, lines, records};

const BUILD: &str = include_str!("fixtures/build-failed-view.log");

#[test]
fn every_line_is_classified_and_none_is_orphan() {
    let all: Vec<_> = lines(BUILD).collect();
    assert_eq!(all.len(), BUILD.lines().count());
    assert!(all.iter().all(|l| !matches!(l, Line::Orphan(_))));
}

#[test]
fn critical_record_carries_the_traceback() {
    let recs = records(BUILD);
    let crit = recs
        .iter()
        .find(|r| r.header.is_some_and(|h| h.level == Level::Critical))
        .expect("critical record");
    let h = crit.header.unwrap();
    assert_eq!(h.logger, "odoo.service.server");
    assert!(crit.has_traceback());
    assert!(
        crit.body
            .iter()
            .any(|l| l.contains("Field \"contract_approval_step\" does not exist"))
    );
    // The traceback ends where the next header starts.
    let next = recs
        .iter()
        .skip_while(|r| r.header != Some(h))
        .nth(1)
        .unwrap();
    assert_eq!(next.header.unwrap().message, "Initiating shutdown");
}

#[test]
fn failures_in_order() {
    let failures: Vec<_> = records(BUILD)
        .into_iter()
        .filter_map(|r| r.header)
        .filter(|h| h.level.is_failure())
        .map(|h| (h.level, h.logger))
        .collect();
    assert_eq!(
        failures,
        [
            (Level::Error, "odoo.registry"),
            (Level::Critical, "odoo.service.server"),
        ]
    );
}

#[test]
fn continuation_lines_inherit_owner() {
    let cont = lines(BUILD)
        .find_map(|l| match l {
            Line::Continuation { owner, text } if text.starts_with("Traceback") => Some(owner),
            _ => None,
        })
        .unwrap();
    assert_eq!(cont.level, Level::Critical);
    assert_eq!(cont.db, Some("acme-main"));
}
