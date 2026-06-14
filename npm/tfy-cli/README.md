# TFY CLI Developer Preview

This npm package installs the `tfy` command for TFY Developer Preview dogfood/testing.

Install preview builds explicitly:

```sh
npm install -g @tfy/cli@preview
```

The npm package name may differ from the command name because the unscoped `tfy` npm package is already occupied. The installed executable remains `tfy`.

This is not GA/production-ready. TFY does not claim private Codex hook interception, provider prompt proxying, editor auto-hooks, or universal terminal interception. Setup success is not token-savings success; route-bound raw/ledger/no-negative/positive-savings evidence is still required.
