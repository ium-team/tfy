# TFY CLI Developer Preview

This npm package installs the `tfy` command for TFY Developer Preview dogfood/testing.

Install preview builds explicitly:

```sh
npm install -g token-fuck-you@preview
```

Install channels use standard npm package specifiers:

- `token-fuck-you` resolves to the most stable release through the npm `latest` dist-tag.
- `token-fuck-you@0.1.1` resolves to that exact version. npm version installs use `@<version>`, not `/v<version>`.
- `token-fuck-you@preview` resolves to the newest developer preview.

The npm package name is `token-fuck-you` and may differ from the command name because the unscoped `tfy` npm package is already occupied. The installed executable remains `tfy`.

This is not GA/production-ready. TFY does not claim private Codex hook interception, provider prompt proxying, editor auto-hooks, or universal terminal interception. Setup success is not token-savings success; route-bound raw/ledger/no-negative/positive-savings evidence is still required.
