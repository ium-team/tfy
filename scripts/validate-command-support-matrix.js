#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const matrixPath = path.join(root, 'docs', 'command-support-matrix.json');
const manifestPath = path.join(root, 'docs', 'command-benchmark-manifest.json');
const provenancePath = path.join(root, 'docs', 'decisions', 'rtk-filter-provenance.md');

function fail(message) {
  console.error(`command support matrix validation failed: ${message}`);
  process.exit(1);
}

function readJson(file) {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch (error) {
    fail(`${path.relative(root, file)} is not valid JSON: ${error.message}`);
  }
}

if (!fs.existsSync(provenancePath)) {
  fail('docs/decisions/rtk-filter-provenance.md is required before RTK-derived filters are implemented');
}

const matrix = readJson(matrixPath);
const manifest = readJson(manifestPath);

if (matrix.schema_version !== 1) fail('matrix schema_version must be 1');
if (!Array.isArray(matrix.rows) || matrix.rows.length === 0) fail('matrix rows must be a non-empty array');
if (!Array.isArray(manifest.measurements)) fail('benchmark manifest measurements must be an array');

const required = [
  'family',
  'rtk_source_kind',
  'rtk_source_ref',
  'strategy_kind',
  'implemented',
  'fixture_verified',
  'failure_evidence_verified',
  'no_negative_verified',
  'raw_recovery_verified',
  'redaction_verified',
  'human_auto_wrapped',
  'agent_route_verified',
  'mcp_route_verified',
  'interactive_risk',
  'claim_status',
  'parity_claim_eligible',
  'benchmark_manifest_id',
  'unsupported_subcases',
  'notes',
];
const bools = new Set([
  'implemented',
  'fixture_verified',
  'failure_evidence_verified',
  'no_negative_verified',
  'raw_recovery_verified',
  'redaction_verified',
  'human_auto_wrapped',
  'agent_route_verified',
  'mcp_route_verified',
  'parity_claim_eligible',
]);
const families = new Set();
const measurementRequired = [
  'id',
  'status',
  'family',
  'fixture_id',
  'tfy_commit',
  'tfy_mode',
  'rtk_commit_or_mode_when_executable',
  'raw_bytes',
  'redacted_raw_bytes',
  'model_visible_bytes',
  'saved_bytes',
  'no_negative_result',
  'raw_recovery_result',
  'redaction_result',
  'missed_evidence_classification',
  'measured_route',
  'unsupported_subcases',
];
const measurementIds = new Set(
  manifest.measurements
    .filter((m) => m.status === 'pass')
    .map((m) => m.id)
    .filter(Boolean),
);
for (const [index, measurement] of manifest.measurements.entries()) {
  for (const field of measurementRequired) {
    if (!(field in measurement)) fail(`benchmark measurement ${index} is missing ${field}`);
  }
  if (!['planned', 'pass', 'fail', 'blocked'].includes(measurement.status)) {
    fail(`benchmark measurement ${measurement.id || index} has invalid status ${measurement.status}`);
  }
  if (!Array.isArray(measurement.unsupported_subcases)) {
    fail(`benchmark measurement ${measurement.id || index}.unsupported_subcases must be an array`);
  }
}


for (const [index, row] of matrix.rows.entries()) {
  for (const field of required) {
    if (!(field in row)) fail(`row ${index} is missing ${field}`);
  }
  if (!row.family || typeof row.family !== 'string') fail(`row ${index} has invalid family`);
  if (families.has(row.family)) fail(`duplicate family: ${row.family}`);
  families.add(row.family);
  for (const field of bools) {
    if (typeof row[field] !== 'boolean') fail(`${row.family}.${field} must be boolean`);
  }
  if (!Array.isArray(row.unsupported_subcases)) fail(`${row.family}.unsupported_subcases must be an array`);
  if (row.human_auto_wrapped && row.interactive_risk !== 'none') {
    fail(`${row.family} cannot be human_auto_wrapped with interactive_risk=${row.interactive_risk}`);
  }
  if (row.parity_claim_eligible) {
    const blockers = [
      'implemented',
      'fixture_verified',
      'failure_evidence_verified',
      'no_negative_verified',
      'raw_recovery_verified',
      'redaction_verified',
      'agent_route_verified',
    ].filter((field) => row[field] !== true);
    if (blockers.length > 0) fail(`${row.family} parity gate missing: ${blockers.join(', ')}`);
    if (!row.benchmark_manifest_id) fail(`${row.family} parity gate requires benchmark_manifest_id`);
    if (!measurementIds.has(row.benchmark_manifest_id)) {
      fail(`${row.family} benchmark_manifest_id ${row.benchmark_manifest_id} has no passing manifest measurement`);
    }
  }
}

const publicDocs = ['README.md', 'docs/TOOL_FEEDBACK.md', 'docs/EVALUATION_GATES.md', 'docs/COMMAND_SUPPORT_MATRIX.md', 'docs/COMMAND_BENCHMARK_MANIFEST.md'];
const blockedPhrases = [/\bRTK parity\b/i, /\bRTK-parity\b/i];
const hasEligible = matrix.rows.some((row) => row.parity_claim_eligible);
if (!hasEligible) {
  for (const rel of publicDocs) {
    const file = path.join(root, rel);
    if (!fs.existsSync(file)) continue;
    const text = fs.readFileSync(file, 'utf8');
    for (const pattern of blockedPhrases) {
      if (pattern.test(text)) {
        fail(`${rel} uses ${pattern} before any row is parity_claim_eligible`);
      }
    }
  }
}

console.log(`validated ${matrix.rows.length} command-support rows; parity-eligible rows: ${matrix.rows.filter((row) => row.parity_claim_eligible).length}`);
