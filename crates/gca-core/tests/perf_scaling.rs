use gca_core::repeated_heading_index_workload;
use std::time::{Duration, Instant};

const ARCHITECTURE_DOC: &str = include_str!("../../../docs/architecture.md");

fn run_workload(threads: usize, rounds_per_thread: usize) -> Duration {
    let start = Instant::now();

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            std::thread::spawn(move || {
                let mut total = 0usize;
                for _ in 0..rounds_per_thread {
                    total =
                        total.saturating_add(repeated_heading_index_workload(ARCHITECTURE_DOC, 12));
                }
                total
            })
        })
        .collect();

    for handle in handles {
        let value = handle.join().expect("workload thread panicked");
        assert!(value > 0, "workload should produce non-zero output");
    }

    start.elapsed()
}

#[test]
fn scaling_should_not_degrade_badly() {
    let t1 = run_workload(1, 24);
    let t4 = run_workload(4, 24);
    let t8 = run_workload(8, 24);
    let under_coverage = std::env::var_os("LLVM_PROFILE_FILE").is_some();
    let t4_limit = if under_coverage { 6.0 } else { 4.0 };
    let t8_limit = if under_coverage { 3.5 } else { 2.5 };

    assert!(
        t4 <= t1.mul_f64(t4_limit),
        "4 threads regressed more than expected: t1={:?}, t4={:?}",
        t1,
        t4
    );

    assert!(
        t8 <= t4.mul_f64(t8_limit),
        "8 threads regressed more than expected: t4={:?}, t8={:?}",
        t4,
        t8
    );
}
