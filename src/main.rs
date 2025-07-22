use std::{
    thread::sleep,
    time::{Duration, Instant},
};

use data::pattern::Pattern;

pub mod data;
fn main() {
    let bpm: u32 = 160;
    let row_count = 16;
    let pattern = Pattern::new(row_count, 1, 4);
    let expected_duration_row = Duration::from_millis(pattern.row_duration_ms(bpm) as u64);
    println!(
        "Expected row duration: {:.2} ms",
        expected_duration_row.as_millis()
    );

    let mut last_instant = Instant::now();

    for i in 1..=row_count {
        sleep(expected_duration_row);

        let now = Instant::now();
        let elapsed = now.duration_since(last_instant);
        last_instant = now;
        println!(
            "Tick: {:2}: elapsed: {:.2} ms \x07",
            i,
            elapsed.as_secs_f64() * 1000.0
        )
    }
}
