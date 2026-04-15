use criterion::{Criterion, criterion_group, criterion_main};
use gca_core::{build_heading_index, repeated_heading_index_workload};

const ARCHITECTURE_DOC: &str = include_str!("../../../docs/architecture.md");

fn bench_heading_index(c: &mut Criterion) {
    let repeated = ARCHITECTURE_DOC.repeat(32);

    c.bench_function("build_heading_index_architecture_doc", |b| {
        b.iter(|| build_heading_index(&repeated))
    });
}

fn bench_repeated_workload(c: &mut Criterion) {
    c.bench_function("repeated_heading_index_workload", |b| {
        b.iter(|| repeated_heading_index_workload(ARCHITECTURE_DOC, 24))
    });
}

criterion_group!(benches, bench_heading_index, bench_repeated_workload);
criterion_main!(benches);
