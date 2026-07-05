#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const matrixPath = path.join(root, 'docs', 'command-support-matrix.json');
const manifestPath = path.join(root, 'docs', 'command-benchmark-manifest.json');

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


const matrix = readJson(matrixPath);
const manifest = readJson(manifestPath);

if (matrix.schema_version !== 1) fail('matrix schema_version must be 1');
if (!Array.isArray(matrix.rows) || matrix.rows.length === 0) fail('matrix rows must be a non-empty array');
if (!Array.isArray(manifest.measurements)) fail('benchmark manifest measurements must be an array');

const required = [
  'family',
  'source_kind',
  'source_ref',
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
  'external_baseline_when_used',
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
const positiveExternalComparisonClaim = /\b(public\s+)?external[-\s](comparison|superiority)\s+claims?\b/i;
const allowedExternalComparisonContext = /\b(blocked|fail(?:s|ed)? closed|requires?|unless|until|gate|not supported|without|additionally requires|must not|may not)\b/i;
const hasEligible = matrix.rows.some((row) => row.parity_claim_eligible);
if (!hasEligible) {
  for (const rel of publicDocs) {
    const file = path.join(root, rel);
    if (!fs.existsSync(file)) continue;
    const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);
    for (const [lineIndex, line] of lines.entries()) {
      if (positiveExternalComparisonClaim.test(line) && !allowedExternalComparisonContext.test(line)) {
        fail(`${rel}:${lineIndex + 1} makes an ungated public external-comparison claim before any row is parity_claim_eligible`);
      }
    }
  }
}

console.log(`validated ${matrix.rows.length} command-support rows; comparison-eligible rows: ${matrix.rows.filter((row) => row.parity_claim_eligible).length}`);
