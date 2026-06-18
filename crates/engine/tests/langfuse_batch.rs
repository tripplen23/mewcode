//! Unit checks for the Langfuse ingestion batch builder.
//!
//! `build_batch` is pure (no network), so we assert the JSON shape Langfuse
//! expects directly: a `trace-create` plus a `generation-create` linked by
//! `traceId`, the session/model/input/output mapped to the documented fields,
//! and a failed turn marked `level: "ERROR"`.

use chrono::{TimeZone, Utc};

use mewcode_engine::langfuse::{TurnReport, build_batch};

fn report(outcome: Result<String, String>) -> TurnReport {
    TurnReport {
        session_id: Some("sess-123".to_string()),
        model: "claude-3-5-sonnet".to_string(),
        mode: "Build".to_string(),
        input: "hello".to_string(),
        outcome,
        start: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        end: Utc.timestamp_opt(1_700_000_002, 0).unwrap(),
    }
}

#[test]
fn success_turn_maps_trace_and_generation() {
    let batch = build_batch(&report(Ok("hi there".to_string())));
    let events = batch["batch"].as_array().expect("batch is an array");
    assert_eq!(events.len(), 2, "one trace event + one generation event");

    let trace = &events[0];
    assert_eq!(trace["type"], "trace-create");
    assert_eq!(trace["body"]["sessionId"], "sess-123");
    assert_eq!(trace["body"]["input"], "hello");
    assert_eq!(trace["body"]["output"], "hi there");

    let generation = &events[1];
    assert_eq!(generation["type"], "generation-create");
    // The generation is linked to the trace it belongs to.
    assert_eq!(generation["body"]["traceId"], trace["body"]["id"]);
    assert_eq!(generation["body"]["model"], "claude-3-5-sonnet");
    assert_eq!(generation["body"]["output"], "hi there");
    assert_eq!(generation["body"]["level"], "DEFAULT");
    // Timings carry through as RFC 3339 strings.
    assert!(
        generation["body"]["startTime"]
            .as_str()
            .unwrap()
            .starts_with("2023-11-")
    );
    assert!(generation["body"]["endTime"].is_string());
}

#[test]
fn failed_turn_is_recorded_as_error() {
    let batch = build_batch(&report(Err("upstream returned 500".to_string())));
    let generation = &batch["batch"][1];

    assert_eq!(generation["body"]["level"], "ERROR");
    assert_eq!(generation["body"]["output"], "upstream returned 500");
    assert_eq!(generation["body"]["statusMessage"], "upstream returned 500");
}

#[test]
fn missing_session_serialises_as_null() {
    let mut r = report(Ok("ok".to_string()));
    r.session_id = None;
    let batch = build_batch(&r);
    assert!(batch["batch"][0]["body"]["sessionId"].is_null());
}

#[test]
fn project_name_parses_health_response() {
    // The shape returned by GET /api/public/projects.
    let body = r#"{"data":[{"id":"abc","name":"mew","organization":{"id":"o","name":"mew"}}]}"#;
    assert_eq!(mewcode_engine::langfuse::project_name(body), "mew");
    // Unexpected shapes fall back rather than panic.
    assert_eq!(mewcode_engine::langfuse::project_name("{}"), "unknown");
    assert_eq!(
        mewcode_engine::langfuse::project_name("not json"),
        "unknown"
    );
}
