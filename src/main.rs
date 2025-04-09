//! Example usage of the orderbook

use orderbook_rs::{OrderBook, benchmarks::benchmark_long_running};

#[cfg(feature = "perf")]
use orderbook_rs::benchmark_orderbook;

fn main() {
    println!("High-Performance Orderbook Demo");
    println!("===============================\n");

    // Run benchmarks
    #[cfg(feature = "perf")]
    benchmark_orderbook();

    println!("\nDo you want to run the long-running benchmark? (y/n)");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    if input.trim().to_lowercase() == "y" {
        // Create a fresh orderbook for the long benchmark
        let mut book = OrderBook::new("BTC-USD", 1_000_000);
        benchmark_long_running(&mut book);
    }
}
