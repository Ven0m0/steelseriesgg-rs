use steelseries_gg::validation::MemorySample;
use std::time::Instant;

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let start = Instant::now();
    let iters = 1000;

    rt.block_on(async {
        for _ in 0..iters {
            let _ = MemorySample::new().await;
        }
    });

    let elapsed = start.elapsed();
    println!("Baseline: {} iterations took {:?}", iters, elapsed);
    println!("Average per iteration: {:?}", elapsed / iters as u32);
}
