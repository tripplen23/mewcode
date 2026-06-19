// Feature: Rig-agent Langfuse tracing
//
// The server exports the Mew-level `chat-turn` generation to Langfuse. Rig also
// emits internal agent/provider spans, but those lack Langfuse IO fields in the
// current Rig version and produce duplicate null-input/null-output observations.

#[test]
fn langfuse_otel_exports_mew_turn_not_rig_internal_spans() {
    let main_src = include_str!("../src/main.rs");

    assert!(
        main_src.contains("rig::agent_chat=off"),
        "Langfuse OTel export should suppress Rig's internal agent chat span"
    );
    assert!(
        main_src.contains("rig::completions=off"),
        "Langfuse OTel export should suppress Rig's internal provider span"
    );
    assert!(main_src.contains("LANGFUSE_BASE_URL"));
    assert!(
        !main_src.contains("LANGFUSE_HOST"),
        "LANGFUSE_BASE_URL is the only Langfuse URL env var"
    );
}
