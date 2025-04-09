//! Benchmarking utilities for the orderbook

#[cfg(feature = "perf")]
use std::time::Instant;

use crate::orderbook::OrderBook;
use crate::types::{Order, OrderType, Side};

/// Benchmark the orderbook with a variety of operations
#[cfg(feature = "perf")]
pub fn benchmark_orderbook() {
    println!("Running orderbook benchmark...");

    // Create an orderbook with capacity for 1 million orders
    let mut book = OrderBook::new("BTC-USD", 1_000_000);

    bench_insertion(&mut book);
    bench_matching(&mut book);
    bench_cancellation(&mut book);
    bench_market_depth(&mut book);
    bench_mixed_workload(&mut book);
}

/// Benchmark order insertion
#[cfg(feature = "perf")]
fn bench_insertion(book: &mut OrderBook) {
    println!("\n>> Testing Limit Order Insertion");

    // Generate test orders
    let order_count = 100_000;
    let mut orders = Vec::with_capacity(order_count);

    // Prepare test data - half buys, half sells
    for i in 0..order_count {
        let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
        let price = if side == Side::Buy {
            // Buy orders from 9990 to 10000
            9990 + (i as u64 % 100)
        } else {
            // Sell orders from 10000 to 10010
            10000 + (i as u64 % 100)
        };

        orders.push(Order::new(i as u64, price, 100, side, OrderType::Limit));
    }

    // Clear the book first
    for i in 0..book.summary().order_count {
        let _ = book.cancel_order(i as u64);
    }

    // Measure insertion time
    let start = Instant::now();
    for order in &orders {
        let _ = book.add_order(order.clone());
    }
    let elapsed = start.elapsed();
    let ops_per_second = order_count as f64 / elapsed.as_secs_f64();

    println!("Inserted {} orders in {:?}", order_count, elapsed);
    println!("Throughput: {:.2} orders/second", ops_per_second);
    println!("Average latency: {:?}", elapsed / order_count as u32);
    println!(
        "Latency per order: {:.2} ns",
        elapsed.as_nanos() as f64 / order_count as f64
    );
}

/// Benchmark order matching
#[cfg(feature = "perf")]
fn bench_matching(book: &mut OrderBook) {
    println!("\n>> Testing Matching Performance");

    // Generate test buy orders that will rest on the book
    let order_count = 10_000;

    // Clear the book first
    for i in 0..book.summary().order_count {
        let _ = book.cancel_order(i as u64);
    }

    // Add buy orders to the book at different price levels
    for i in 0..order_count {
        let price = 9500 + (i % 100) as u64; // Prices from 9500 to 9599
        let order = Order::new(i as u64, price, 100, Side::Buy, OrderType::Limit);

        let _ = book.add_order(order);
    }

    // Now create sell orders that will match against the book
    let match_count = 1_000;
    let mut match_orders = Vec::with_capacity(match_count);

    for i in 0..match_count {
        let price = 9450; // Price below all buy orders, ensuring matches
        let order = Order::new(
            (order_count + i) as u64,
            price,
            100,
            Side::Sell,
            OrderType::Limit,
        );

        match_orders.push(order);
    }

    // Measure matching time
    let start = Instant::now();
    let mut total_executions = 0;

    for order in &match_orders {
        if let Ok(executions) = book.add_order(order.clone()) {
            total_executions += executions.len();
        }
    }

    let elapsed = start.elapsed();
    let ops_per_second = match_count as f64 / elapsed.as_secs_f64();

    println!(
        "Matched {} orders in {:?}, creating {} executions",
        match_count, elapsed, total_executions
    );
    println!("Throughput: {:.2} orders/second", ops_per_second);
    println!("Average latency: {:?}", elapsed / match_count as u32);
    println!(
        "Latency per order: {:.2} ns",
        elapsed.as_nanos() as f64 / match_count as f64
    );

    if total_executions > 0 {
        println!(
            "Latency per execution: {:.2} ns",
            elapsed.as_nanos() as f64 / total_executions as f64
        );
    } else {
        println!("ERROR: No executions created during matching benchmark!");
    }
}

/// Benchmark order cancellation
#[cfg(feature = "perf")]
fn bench_cancellation(book: &mut OrderBook) {
    println!("\n>> Testing Cancellation Performance");

    // Generate test orders
    let order_count = 100_000;

    // Clear the book first
    for i in 0..book.summary().order_count {
        let _ = book.cancel_order(i as u64);
    }

    // Add orders to the book
    for i in 0..order_count {
        let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
        let price = if side == Side::Buy {
            99_900 + (i as u64 % 100)
        } else {
            100_000 + (i as u64 % 100)
        };

        let order = Order::new(i as u64, price, 100, side, OrderType::Limit);

        let _ = book.add_order(order);
    }

    // Measure cancellation time
    let start = Instant::now();
    for i in 0..order_count {
        let _ = book.cancel_order(i as u64);
    }
    let elapsed = start.elapsed();
    let ops_per_second = order_count as f64 / elapsed.as_secs_f64();

    println!("Cancelled {} orders in {:?}", order_count, elapsed);
    println!("Throughput: {:.2} orders/second", ops_per_second);
    println!("Average latency: {:?}", elapsed / order_count as u32);
    println!(
        "Latency per cancellation: {:.2} ns",
        elapsed.as_nanos() as f64 / order_count as f64
    );
}

/// Benchmark market depth retrieval
#[cfg(feature = "perf")]
fn bench_market_depth(book: &mut OrderBook) {
    println!("\n>> Testing Market Depth Retrieval");

    // Generate test orders
    let order_count = 10_000;

    // Clear the book first
    for i in 0..book.summary().order_count {
        let _ = book.cancel_order(i as u64);
    }

    // Add orders to the book across many price levels
    for i in 0..order_count {
        let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
        let price = if side == Side::Buy {
            99_000 + (i as u64 % 1000)
        } else {
            100_000 + (i as u64 % 1000)
        };

        let order = Order::new(i as u64, price, 100, side, OrderType::Limit);

        let _ = book.add_order(order);
    }

    // Measure market depth retrieval time
    let iterations = 10_000;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = book.market_depth(10);
    }

    let elapsed = start.elapsed();
    let ops_per_second = iterations as f64 / elapsed.as_secs_f64();

    println!(
        "Retrieved market depth {} times in {:?}",
        iterations, elapsed
    );
    println!("Throughput: {:.2} retrievals/second", ops_per_second);
    println!("Average latency: {:?}", elapsed / iterations as u32);
    println!(
        "Latency per retrieval: {:.2} ns",
        elapsed.as_nanos() as f64 / iterations as f64
    );
}

/// Benchmark a mixed workload simulating realistic market activity
#[cfg(feature = "perf")]
fn bench_mixed_workload(book: &mut OrderBook) {
    println!("\n>> Testing Mixed Workload Performance");

    // Clear the book first
    for i in 0..book.summary().order_count {
        let _ = book.cancel_order(i as u64);
    }

    // Parameters for the test
    let total_operations = 100_000;
    let insert_ratio = 0.7; // 70% insertions
    let cancel_ratio = 0.2; // 20% cancellations

    let mut next_order_id = 0;
    let mut live_orders = Vec::new();

    // Measure mixed workload time
    let start = Instant::now();

    for i in 0..total_operations {
        let op_type = {
            let r = i as f64 / total_operations as f64;
            if r < insert_ratio {
                0 // Insert
            } else if r < insert_ratio + cancel_ratio {
                1 // Cancel
            } else {
                2 // Market
            }
        };

        match op_type {
            0 => {
                // Insert a limit order
                let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
                let jitter = (i % 20) as u64;

                let price = if side == Side::Buy {
                    99_90 + jitter
                } else {
                    100_00 + jitter
                };

                let order = Order::new(next_order_id, price, 100 + jitter, side, OrderType::Limit);

                if let Ok(_) = book.add_order(order) {
                    live_orders.push(next_order_id);
                    next_order_id += 1;
                }
            }
            1 => {
                // Cancel an order
                if !live_orders.is_empty() {
                    let idx = i % live_orders.len();
                    let order_id = live_orders[idx];
                    let _ = book.cancel_order(order_id);
                    live_orders.swap_remove(idx);
                }
            }
            2 => {
                // Submit a market order
                let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
                let quantity = 100 + (i % 10) as u64;

                let order = Order::new(
                    next_order_id,
                    0, // Price doesn't matter for market orders
                    quantity,
                    side,
                    OrderType::Market,
                );

                let _ = book.add_order(order);
                next_order_id += 1;
            }
            _ => unreachable!(),
        }
    }

    let elapsed = start.elapsed();
    let ops_per_second = total_operations as f64 / elapsed.as_secs_f64();

    println!(
        "Completed {} mixed operations in {:?}",
        total_operations, elapsed
    );
    println!("Throughput: {:.2} operations/second", ops_per_second);
    println!("Average latency: {:?}", elapsed / total_operations as u32);
    println!(
        "Latency per operation: {:.2} ns",
        elapsed.as_nanos() as f64 / total_operations as f64
    );

    // Print final orderbook state
    let summary = book.summary();
    println!("\nFinal orderbook state:\n{}", summary);
}

/// Run a long-running benchmark (minimum 1 minute) with a mixed workload
pub fn benchmark_long_running(book: &mut OrderBook) {
    println!("\n>> Starting Long-Running Mixed Workload Benchmark (1+ minute)");
    println!("This benchmark simulates realistic market activity under sustained load");

    // Parameters for the test
    let min_runtime_secs = 60; // Run for at least 60 seconds
    let max_orders = 20_000_000; // Safety cap to prevent infinite loop

    // Ratios for different operations
    let insert_ratio = 0.65; // 65% insertions
    let cancel_ratio = 0.20; // 20% cancellations
    let market_ratio = 0.10; // 10% market orders

    // Price ranges for orders (adjusted for base price of 10,000)
    let min_price = 9000;
    let max_price = 11000;
    let price_levels = 200; // Number of distinct price levels to use

    // Tracking variables
    let mut next_order_id: u64 = 0;
    let mut live_orders = Vec::with_capacity(1_000_000); // Orders that can be cancelled
    let mut total_operations = 0;
    let mut total_inserts = 0;
    let mut total_cancellations = 0;
    let mut total_market_orders = 0;
    let mut total_queries = 0;
    let mut total_executions = 0;

    // Statistics by operation type
    let mut insert_time = std::time::Duration::new(0, 0);
    let mut cancel_time = std::time::Duration::new(0, 0);
    let mut market_time = std::time::Duration::new(0, 0);
    let mut query_time = std::time::Duration::new(0, 0);

    // Seed the book with some initial orders
    let seed_count = 10_000;
    for i in 0..seed_count {
        let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
        let price_offset = (i % price_levels) as u64;

        let price = if side == Side::Buy {
            min_price + price_offset * ((max_price - min_price) / price_levels as u64)
        } else {
            max_price - price_offset * ((max_price - min_price) / price_levels as u64)
        };

        let quantity = 100 + (i % 10) * 10;

        let order = Order::new(next_order_id, price, quantity, side, OrderType::Limit);

        if let Ok(_) = book.add_order(order) {
            live_orders.push(next_order_id);
            next_order_id += 1;
            total_operations += 1;
            total_inserts += 1;
        }
    }

    println!("Seeded orderbook with {} initial orders", seed_count);
    println!("Starting main benchmark loop...");

    // Track start time for overall benchmark
    let benchmark_start = std::time::Instant::now();
    let mut last_report = benchmark_start;
    let report_interval = std::time::Duration::from_secs(5); // Report every 5 seconds

    // Main benchmark loop
    while benchmark_start.elapsed().as_secs() < min_runtime_secs && total_operations < max_orders {
        // Determine operation type
        let op_type = {
            let r = rand::random::<f64>();
            if r < insert_ratio {
                0 // Insert
            } else if r < insert_ratio + cancel_ratio {
                1 // Cancel
            } else if r < insert_ratio + cancel_ratio + market_ratio {
                2 // Market
            } else {
                3 // Query
            }
        };

        match op_type {
            0 => {
                // Insert a limit order
                let side = if rand::random::<bool>() {
                    Side::Buy
                } else {
                    Side::Sell
                };
                let price_offset = rand::random::<u64>() % price_levels as u64;

                let price = if side == Side::Buy {
                    min_price + price_offset * ((max_price - min_price) / price_levels as u64)
                } else {
                    max_price - price_offset * ((max_price - min_price) / price_levels as u64)
                };

                let quantity = 100 + (rand::random::<u64>() % 10) * 10;

                let order = Order::new(next_order_id, price, quantity, side, OrderType::Limit);

                let start = std::time::Instant::now();
                if let Ok(executions) = book.add_order(order) {
                    insert_time += start.elapsed();
                    live_orders.push(next_order_id);
                    next_order_id += 1;
                    total_operations += 1;
                    total_inserts += 1;
                    total_executions += executions.len();
                }
            }
            1 => {
                // Cancel an order
                if !live_orders.is_empty() {
                    let idx = (rand::random::<u64>() as usize) % live_orders.len();
                    let order_id = live_orders[idx];

                    let start = std::time::Instant::now();
                    if let Ok(_) = book.cancel_order(order_id) {
                        cancel_time += start.elapsed();
                        live_orders.swap_remove(idx);
                        total_operations += 1;
                        total_cancellations += 1;
                    }
                } else {
                    // If no orders to cancel, insert instead
                    total_operations -= 1; // Will be incremented again in next loop
                    continue;
                }
            }
            2 => {
                // Submit a market order
                let side = if rand::random::<bool>() {
                    Side::Buy
                } else {
                    Side::Sell
                };
                let quantity = 100 + (rand::random::<u64>() % 20) * 10; // Slightly larger for market orders

                let order = Order::new(
                    next_order_id,
                    0, // Price doesn't matter for market orders
                    quantity,
                    side,
                    OrderType::Market,
                );

                let start = std::time::Instant::now();
                if let Ok(executions) = book.add_order(order) {
                    market_time += start.elapsed();
                    next_order_id += 1;
                    total_operations += 1;
                    total_market_orders += 1;
                    total_executions += executions.len();
                }
            }
            3 => {
                // Query market depth
                let depth = 10 + (rand::random::<u64>() % 10); // Random depth between 10-20 levels

                let start = std::time::Instant::now();
                let _ = book.market_depth(depth as usize);
                query_time += start.elapsed();
                total_operations += 1;
                total_queries += 1;
            }
            _ => unreachable!(),
        }

        // Periodic reporting
        if last_report.elapsed() >= report_interval {
            last_report = std::time::Instant::now();
            let elapsed = benchmark_start.elapsed();
            let ops_per_sec = total_operations as f64 / elapsed.as_secs_f64();

            println!(
                "Progress: {:.1}s elapsed, {} operations, {:.2} ops/sec",
                elapsed.as_secs_f64(),
                total_operations,
                ops_per_sec,
            );
        }
    }

    // Final timing and statistics
    let elapsed = benchmark_start.elapsed();
    let total_time_ns = elapsed.as_nanos();
    let ops_per_second = total_operations as f64 / elapsed.as_secs_f64();

    // Calculate average latencies
    let avg_insert_ns = if total_inserts > 0 {
        insert_time.as_nanos() as f64 / total_inserts as f64
    } else {
        0.0
    };

    let avg_cancel_ns = if total_cancellations > 0 {
        cancel_time.as_nanos() as f64 / total_cancellations as f64
    } else {
        0.0
    };

    let avg_market_ns = if total_market_orders > 0 {
        market_time.as_nanos() as f64 / total_market_orders as f64
    } else {
        0.0
    };

    let avg_query_ns = if total_queries > 0 {
        query_time.as_nanos() as f64 / total_queries as f64
    } else {
        0.0
    };

    let avg_execution_ns = if total_executions > 0 {
        (insert_time.as_nanos() + market_time.as_nanos()) as f64 / total_executions as f64
    } else {
        0.0
    };

    // Print detailed results
    println!(
        "\n>> Long-Running Benchmark Results ({:.2} seconds)",
        elapsed.as_secs_f64()
    );
    println!("Total operations: {}", total_operations);
    println!(
        "Overall throughput: {:.2} operations/second",
        ops_per_second
    );
    println!(
        "Average latency: {:.2} ns/operation",
        total_time_ns as f64 / total_operations as f64
    );

    println!("\nOperation breakdown:");
    println!(
        "  Inserts: {} ({:.1}%), avg {:.2} ns",
        total_inserts,
        (total_inserts as f64 / total_operations as f64) * 100.0,
        avg_insert_ns
    );
    println!(
        "  Cancellations: {} ({:.1}%), avg {:.2} ns",
        total_cancellations,
        (total_cancellations as f64 / total_operations as f64) * 100.0,
        avg_cancel_ns
    );
    println!(
        "  Market orders: {} ({:.1}%), avg {:.2} ns",
        total_market_orders,
        (total_market_orders as f64 / total_operations as f64) * 100.0,
        avg_market_ns
    );
    println!(
        "  Market depth queries: {} ({:.1}%), avg {:.2} ns",
        total_queries,
        (total_queries as f64 / total_operations as f64) * 100.0,
        avg_query_ns
    );

    println!("\nExecution statistics:");
    println!("  Total executions: {}", total_executions);
    if total_executions > 0 {
        println!("  Avg latency per execution: {:.2} ns", avg_execution_ns);
        println!(
            "  Executions per market order: {:.2}",
            total_executions as f64 / total_market_orders as f64
        );
    }

    println!("\nOrderbook statistics:");
    println!("  Final orderbook state:\n{}", book.summary());
}
