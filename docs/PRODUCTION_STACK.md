# TFY Production Stack

TFY's Python implementation is the reference prototype. The production implementation is now prepared as a Rust-first workspace.

## Final stack

- Core: Rust (`crates/tfy-core`)
- Parser: Tree-sitter grammars
- CLI: Rust binary (`crates/tfy-cli`, binary name `tfy`)
- Python binding scaffold: PyO3/maturin (`bindings/tfy-python`)
- Node/Wasm: deferred until Rust CLI/core parity is stable

## Production invariants

- CLI-first JSON protocol remains the canonical agent-neutral surface.
- Parser-backed responses include `parser`, `confidence`, `fallback_action`, and `reason`.
- First-wave production adapters are Python, JavaScript, JSX, TypeScript, TSX, Rust, and Go.
- C-family remains experimental until corpus/confidence gates are met.
- Python prototype fixtures under `oracle/fixtures/` are migration oracles, not a permanent runtime dependency.
- Bindings call Rust core and do not duplicate transformation logic.

## Rust commands

```sh
cargo run -p tfy-cli -- languages
cargo run -p tfy-cli -- index examples/sample.py
cargo run -p tfy-cli -- expand examples/sample.py sample.py:calculate_total_price:1 --compactness symbol
cargo run -p tfy-cli -- restore --payload payload.json
cargo run -p tfy-cli -- run -- python3 -c 'print("ok")'
cargo run -p tfy-cli -- raw <raw_ref>
cargo run -p tfy-cli -- eval-code examples/sample.py sample.py:calculate_total_price:1
```

## Verification

```sh
cargo test --workspace
cargo check --workspace
cargo bench -p tfy-core --bench core_bench
PYTHONPATH=src python3 -m unittest discover -s tests -v
```
