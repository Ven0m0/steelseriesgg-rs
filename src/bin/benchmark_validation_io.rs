use std::time::Instant;
use steelseries_gg::validation::MemorySample;

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let start = Instant::now();
    let iters = 1000;

    let benchmark_result = rt.block_on(async {
        for iteration in 0..iters {
            if let Err(err) = MemorySample::new().await {
                return Err((iteration, err));
            }
        }
        Ok(())
    });

    if let Err((iteration, err)) = benchmark_result {
        eprintln!(
            "Benchmark aborted after {} successful iterations: failed to collect memory sample: {}",
            iteration, err
        );
        return;
    }

    let elapsed = start.elapsed();
    println!("Baseline: {} iterations took {:?}", iters, elapsed);
    println!("Average per iteration: {:?}", elapsed / iters as u32);
}
