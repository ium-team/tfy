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
fn legacy_tool_command_event_deserializes_without_rule_metadata() {
    let json = serde_json::json!({
        "kind": "tool_command_completed",
        "command": "printf ok",
        "exit_code": 0,
        "risk": "success",
        "command_family": "generic",
        "strategy_kind": "generic",
        "human_auto_safe": false,
        "agent_safe": true,
        "interactive_risk": "unknown",
        "raw_ref": "cmdout_deadbeef0000_0000000000000000",
        "raw_bytes": 2,
        "model_bytes": 2,
        "raw_chars": 2,
        "summary_chars": 2,
        "model_chars": 2,
        "savings_pct": 0.0,
        "negative_savings_avoided": false,
        "rendering_kind": "pass_through",
        "output_sha256": "sha"
    });
    let event: GatewayEvent = serde_json::from_value(json).unwrap();
    let GatewayEvent::ToolCommandCompleted {
        rule_id,
        strategy_source_kind,
        command_rule_diagnostics,
        ..
    } = event
    else {
        panic!("expected tool command event");
    };
    assert_eq!(rule_id, None);
    assert!(strategy_source_kind.is_empty());
    assert!(command_rule_diagnostics.is_empty());
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
            command_family: "cargo_test".into(),
            strategy_kind: "rust".into(),
            human_auto_safe: false,
            agent_safe: true,
            interactive_risk: "none".into(),
            rule_id: None,
            strategy_source_kind: "built_in".into(),
            command_rule_diagnostics: Vec::new(),
            raw_ref: "cmdout_deadbeef0000_0000000000000000".into(),
            raw_bytes: 100,
            model_bytes: 42,
            raw_chars: 100,
            summary_chars: 42,
            model_chars: 42,
            savings_pct: 58.0,
            negative_savings_avoided: false,
            rendering_kind: "summary".into(),
            output_sha256: "test-output-sha256".into(),
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
            command_family: "cargo_test".into(),
            strategy_kind: "rust".into(),
            human_auto_safe: false,
            agent_safe: true,
            interactive_risk: "none".into(),
            rule_id: None,
            strategy_source_kind: "built_in".into(),
            command_rule_diagnostics: Vec::new(),
            raw_ref: "cmdout_deadbeef0000_0000000000000000".into(),
            raw_bytes: 100,
            model_bytes: 42,
            raw_chars: 100,
            summary_chars: 42,
            model_chars: 42,
            savings_pct: 58.0,
            negative_savings_avoided: false,
            rendering_kind: "summary".into(),
            output_sha256: "test-output-sha256".into(),
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
            command_family: "cargo_test".into(),
            strategy_kind: "rust".into(),
            human_auto_safe: false,
            agent_safe: true,
            interactive_risk: "none".into(),
            rule_id: None,
            strategy_source_kind: "built_in".into(),
            command_rule_diagnostics: Vec::new(),
            raw_ref: "cmdout_deadbeef0000_0000000000000000".into(),
            raw_bytes: 100,
            model_bytes: 42,
            raw_chars: 100,
            summary_chars: 42,
            model_chars: 42,
            savings_pct: 58.0,
            negative_savings_avoided: false,
            rendering_kind: "summary".into(),
            output_sha256: "test-output-sha256".into(),
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

#[test]
fn command_rule_diagnostic_deserializes_from_partial_object() {
    let diagnostic: tfy_runtime::CommandRuleDiagnostic =
        serde_json::from_str(r#"{"code":"repo_rules_untrusted"}"#).unwrap();
    assert_eq!(diagnostic.code, "repo_rules_untrusted");
    assert!(diagnostic.source_kind.is_empty());
    assert!(diagnostic.path.is_empty());
    assert!(diagnostic.message.is_empty());
}

#[test]
fn ledger_events_with_removed_route_tags_remain_readable_as_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let line = serde_json::json!({
        "protocol_version": RUNTIME_PROTOCOL_VERSION,
        "adapter_kind": "legacy_removed_adapter",
        "adapter_version": "0.0.0",
        "supported_gateways": ["tool"],
        "authority_mode": "execute_with_runtime_authority",
        "session_id": "legacy-session",
        "turn_id": null,
        "request_id": "legacy-request",
        "parent_event_id": null,
        "trace_id": "legacy-trace",
        "workspace_root": null,
        "origin": {
            "kind": "legacy_removed_origin",
            "host": "generic",
            "invocation": "legacy_removed_invocation",
            "intercepted": true,
            "user_shell_mutated": false
        },
        "route": {
            "ingress": "legacy_removed_route",
            "host": "generic",
            "claim_tier": "legacy_removed_claim"
        },
        "provenance": {"raw_refs": ["cmdout_deadbeef0000_0000000000000000"], "validation_status": "valid"},
        "policy": RuntimePolicy::default(),
        "payload": {
            "kind": "tool_command_completed",
            "command": "printf ok",
            "exit_code": 0,
            "risk": "success",
            "raw_ref": "cmdout_deadbeef0000_0000000000000000"
        }
    });
    std::fs::write(&ledger, format!("{}\n", line)).unwrap();

    let events = load_events(&ledger).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].adapter_kind, AdapterKind::Unknown);
    assert_eq!(events[0].origin.kind, OriginKind::Unknown);
    assert_eq!(events[0].origin.invocation, OriginInvocation::Unknown);
    assert_eq!(events[0].route.ingress, RouteIngressKind::Unknown);
    assert_eq!(events[0].route.claim_tier, RouteClaimTier::Unknown);
}
