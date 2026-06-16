# Command Benchmark Manifest

`docs/command-benchmark-manifest.json` is the machine-readable evidence gate for public command-output comparison claims.

A support-matrix row may not set `parity_claim_eligible=true` unless the benchmark manifest contains a matching measurement with:

- command family and fixture id;
- TFY commit and mode;
- RTK commit/mode when executable, or an explicit non-executable reason;
- raw bytes, redacted raw bytes, model-visible bytes, and saved bytes;
- no-negative selector result;
- raw recovery result;
- redaction result where relevant;
- missed-evidence classification;
- measured route; and
- unsupported or unsafe subcases.

The initial manifest contains only `planned` rows. Planned rows reserve the fixture/route evidence shape for implementation work, but only `status=pass` measurements can unlock public comparison claims.
