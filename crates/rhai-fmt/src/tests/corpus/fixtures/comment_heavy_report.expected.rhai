import "reporting" as reporting;
// keep reporting first because it patches severity labels
import "alerts" as alerts;

/// build the daily report
fn build_report(items, owner) {
    // derive the visible rows
    let rows = [
        // include first
        items[0],
        items?[1], // safe backup
    ];

    let summary = switch rows.len() {
        // empty case
        0 => `empty`,
        // single row
        1 => {
            // keep the raw item name
            rows[0].name
        },
        // fallback
        _ => `many:${rows.len()}`
    };

    reporting::emit(
        rows, // current rows
        summary,
        // escalation owner
        owner,
    );
}
