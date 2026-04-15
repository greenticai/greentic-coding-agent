use gca_core::repeated_heading_index_workload;
use std::time::{Duration, Instant};

const ARCHITECTURE_DOC: &str = include_str!("../../../docs/architecture.md");

#[test]
fn workload_should_finish_quickly() {
    let start = Instant::now();
    let total = repeated_heading_index_workload(ARCHITECTURE_DOC, 32);
    let elapsed = start.elapsed();

    assert!(total > 0, "workload should produce output");
    assert!(
        elapsed < Duration::from_secs(2),
        "workload too slow: {:?}",
        elapsed
    );
}
