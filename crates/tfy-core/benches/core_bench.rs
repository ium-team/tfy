use criterion::{criterion_group, criterion_main, Criterion};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bench_index(c: &mut Criterion) {
    c.bench_function("index_python_sample", |b| {
        b.iter(|| tfy_core::index_path(root().join("examples/sample.py")).unwrap())
    });
}

fn bench_expand(c: &mut Criterion) {
    c.bench_function("expand_python_sample", |b| {
        b.iter(|| {
            tfy_core::expand_scope(
                root().join("examples/sample.py"),
                "sample.py:calculate_total_price:1",
                "symbol",
            )
            .unwrap()
        })
    });
}

criterion_group!(benches, bench_index, bench_expand);
criterion_main!(benches);
