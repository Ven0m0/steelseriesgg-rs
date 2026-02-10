use std::hint::black_box;
use std::time::Instant;
use steelseries_gg::rgb::Color;

fn main() {
    let iterations = 10_000_000;
    let zone_count = 104;

    // Create input data
    let input_colors: Vec<Color> = (0..zone_count / 2).map(|i| Color::new((i % 255) as u8, 0, 0)).collect();

    // 1. Benchmark Allocation Strategy (Current)
    let start_alloc = Instant::now();

    for _ in 0..iterations {
        // Original logic
        let mut zone_colors = input_colors.iter().take(zone_count).copied().collect::<Vec<_>>();

        while zone_colors.len() < zone_count {
            zone_colors.push(Color::BLACK);
        }

        black_box(zone_colors.as_ptr());
    }

    let duration_alloc = start_alloc.elapsed();
    println!("Allocation Strategy:       {:?}", duration_alloc);

    // 2. Benchmark Reuse (Iterator)
    let start_reuse_iter = Instant::now();
    let mut buffer = Vec::with_capacity(zone_count);

    for _ in 0..iterations {
        buffer.clear();
        // Mimic iterator usage
        buffer.extend(input_colors.iter().take(zone_count).copied());

        while buffer.len() < zone_count {
            buffer.push(Color::BLACK);
        }

        black_box(buffer.as_ptr());
    }

    let duration_reuse_iter = start_reuse_iter.elapsed();
    println!("Reuse Strategy (Iter):     {:?}", duration_reuse_iter);

    // 3. Benchmark Reuse (Slice) - Optimized
    let start_reuse_slice = Instant::now();
    let mut buffer2 = Vec::with_capacity(zone_count);

    for _ in 0..iterations {
        buffer2.clear();
        // Optimized copy
        let len = input_colors.len().min(zone_count);
        buffer2.extend_from_slice(&input_colors[..len]);

        while buffer2.len() < zone_count {
            buffer2.push(Color::BLACK);
        }

        black_box(buffer2.as_ptr());
    }

    let duration_reuse_slice = start_reuse_slice.elapsed();
    println!("Reuse Strategy (Slice):    {:?}", duration_reuse_slice);

    let speedup = duration_alloc.as_secs_f64() / duration_reuse_slice.as_secs_f64();
    println!("Speedup (Alloc vs Slice): {:.2}x", speedup);
}
