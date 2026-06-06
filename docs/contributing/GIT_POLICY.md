# TFY Git Policy

This is the concrete branch, commit, and PR policy for TFY.

## Branch naming

Use one of these prefixes:

| Prefix | Use for | Example |
|---|---|---|
| `feat/` | User-visible product feature | `feat/mcp-resource-cache` |
| `fix/` | Bug fix | `fix/raw-ref-invalid-utf8` |
| `docs/` | Documentation-only change | `docs/adapter-claims` |
| `refactor/` | Internal structure change without intended behavior change | `refactor/cli-modules` |
| `test/` | Test-only or fixture-only work | `test/mcp-session-scope` |
| `chore/` | CI, release, dependency, or repo hygiene | `chore/github-templates` |
| `codex/` | AI-agent-authored branch when no better branch exists yet | `codex/rust-only-runtime` |

Rules:

- Prefer one purpose per branch.
- If a branch already exists for an open PR, continue it only when the new work belongs to that PR's story.
- Do not force-push shared branches unless explicitly requested.
- Do not create branches named only `main`, `master`, `temp`, `test`, or `fix`.

## Commit messages

Use Lore commit messages.

Required shape:

```text
<intent line: why the change exists>

<optional short body>

Constraint: <constraint>
Rejected: <alternative> | <reason>
Confidence: <low|medium|high>
Scope-risk: <narrow|moderate|broad>
Directive: <future warning>
Tested: <commands/evidence>
Not-tested: <known gaps>
```

Good example:

```text
Keep TFY adapter claims auditable

Codex setup remains MCP-routed, so the docs and CLI output must not imply private hook interception.

Constraint: Current Codex support is a dry-run MCP setup snippet.
Rejected: Claiming automatic Codex interception | no private hook adapter exists or has e2e tests.
Confidence: high
Scope-risk: narrow
Directive: Do not expand support claims without host-level tests.
Tested: cargo test -p tfy-cli --test mcp_server
Not-tested: Live mutation of a user Codex config.
```

Bad examples:

- `update files`
- `fix stuff`
- `WIP`
- `agent changes`

## Pull request expectations

Every PR should include:

- summary of what changed
- supported boundary and unsupported claims
- user/developer impact
- validation commands
- risk notes

Draft PR is preferred until CI is green and support claims are reviewed.
