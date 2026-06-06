use tfy_runtime::*;

#[test]
fn runtime_envelope_round_trips_with_required_fields() {
    let env = RuntimeEnvelope::new(
        AdapterKind::TestHarness,
        vec![GatewayKind::Tool, GatewayKind::State],
        AuthorityMode::ExecuteWithRuntimeAuthority,
        "session-1",
        "request-1",
        "trace-1",
        GatewayRequest::ToolCommand {
            command: vec!["printf".into(), "ok".into()],
        },
    );
    validate_envelope(&env).unwrap();
    let json = serde_json::to_string(&env).unwrap();
    assert!(json.contains("protocol_version"));
    assert!(json.contains("request_id"));
    assert!(json.contains("trace_id"));
    let decoded: RuntimeEnvelope<GatewayRequest> = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.protocol_version, RUNTIME_PROTOCOL_VERSION);
    assert_eq!(decoded.session_id, "session-1");
}

#[test]
fn negotiation_degrades_on_boundary_mismatches() {
    let caps = AdapterCapabilities::cli_default();
    let bad_version = NegotiationRequest {
        required_protocol_version: "9.9.9".into(),
        required_gateways: vec![GatewayKind::Tool],
        required_output_mode: OutputMode::Json,
        minimum_authority_mode: AuthorityMode::ObserveOnly,
        require_redaction: true,
        require_raw_store: true,
        adapter_enabled: true,
    };
    let result = negotiate(&caps, &bad_version);
    assert!(!result.accepted);
    assert_eq!(
        result.fallback_reason,
        Some(FallbackReason::UnsupportedProtocolVersion)
    );

    let disabled = NegotiationRequest {
        adapter_enabled: false,
        ..bad_version
    };
    let result = negotiate(&caps, &disabled);
    assert_eq!(
        result.fallback_reason,
        Some(FallbackReason::AdapterDisabled)
    );
}

#[test]
fn state_projection_with_valid_lineage_is_authoritative() {
    let event = event_envelope(
        GatewayEvent::ToolCommandCompleted {
            command: "cargo test".into(),
            exit_code: 0,
            risk: "success".into(),
            raw_ref: "cmdout_deadbeef0000_0000000000000000".into(),
            raw_bytes: 100,
            model_bytes: 42,
            raw_chars: 100,
            summary_chars: 42,
            model_chars: 42,
            savings_pct: 58.0,
            negative_savings_avoided: false,
            rendering_kind: "summary".into(),
        },
        "s",
        "r",
        "t",
        None,
        ProvenanceRefs {
            raw_refs: vec!["cmdout_deadbeef0000_0000000000000000".into()],
            validation_status: Some(ValidationStatus::Valid),
            ..Default::default()
        },
    );
    let projection = project_state(&[event]);
    assert!(projection.authoritative);
    assert_eq!(
        projection.provenance.validation_status,
        Some(ValidationStatus::Valid)
    );
    assert_eq!(projection.fallback_reason, None);
}

#[test]
fn state_projection_without_lineage_is_non_authoritative() {
    let event = event_envelope(
        GatewayEvent::ToolCommandCompleted {
            command: "cargo test".into(),
            exit_code: 0,
            risk: "success".into(),
            raw_ref: "cmdout_deadbeef0000_0000000000000000".into(),
            raw_bytes: 100,
            model_bytes: 42,
            raw_chars: 100,
            summary_chars: 42,
            model_chars: 42,
            savings_pct: 58.0,
            negative_savings_avoided: false,
            rendering_kind: "summary".into(),
        },
        "s",
        "r",
        "t",
        None,
        ProvenanceRefs::default(),
    );
    let projection = project_state(&[event]);
    assert!(!projection.authoritative);
    assert_eq!(
        projection.fallback_reason,
        Some(FallbackReason::MissingProvenance)
    );
    assert!(projection.tool_evidence[0].contains("raw_ref="));
}

#[test]
fn state_projection_with_mixed_validity_is_non_authoritative() {
    let valid = event_envelope(
        GatewayEvent::ToolCommandCompleted {
            command: "cargo test".into(),
            exit_code: 0,
            risk: "success".into(),
            raw_ref: "cmdout_deadbeef0000_0000000000000000".into(),
            raw_bytes: 100,
            model_bytes: 42,
            raw_chars: 100,
            summary_chars: 42,
            model_chars: 42,
            savings_pct: 58.0,
            negative_savings_avoided: false,
            rendering_kind: "summary".into(),
        },
        "s",
        "valid-event",
        "t",
        None,
        ProvenanceRefs {
            raw_refs: vec!["cmdout_deadbeef0000_0000000000000000".into()],
            validation_status: Some(ValidationStatus::Valid),
            ..Default::default()
        },
    );
    let invalid = event_envelope(
        GatewayEvent::OutputValidated {
            patch_ref: "patch_1".into(),
            validation_status: ValidationStatus::NonAuthoritative,
            applied: false,
        },
        "s",
        "invalid-event",
        "t",
        Some("valid-event".into()),
        ProvenanceRefs {
            patch_refs: vec!["patch_1".into()],
            validation_status: Some(ValidationStatus::NonAuthoritative),
            ..Default::default()
        },
    );
    let projection = project_state(&[valid, invalid]);
    assert!(!projection.authoritative);
    assert_eq!(
        projection.provenance.validation_status,
        Some(ValidationStatus::NonAuthoritative)
    );
    assert_eq!(
        projection.fallback_reason,
        Some(FallbackReason::MissingProvenance)
    );
}
