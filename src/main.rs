//! Example usage of the orderbook

use orderbook_rs::{
    benchmark_orderbook, benchmarks::benchmark_long_running, Order, OrderBook, OrderType, Side,
};

fn main() {
    println!("High-Performance Orderbook Demo");
    println!("===============================\n");

    // Simple example usage
    simple_example();

    // Run benchmarks
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

/// Simple example showing basic orderbook usage
fn simple_example() {
    println!("Running simple example...\n");

    // Create an orderbook
    let mut book = OrderBook::new("BTC-USD", 10_000);

    // Add some buy orders
    for i in 0..5 {
        let price = 9900 + i * 10;
        let qty = 100 + i * 50;

        let order = Order::new(i, price, qty, Side::Buy, OrderType::Limit);

        if let Ok(executions) = book.add_order(order) {
            println!("Added buy order id={} price={} qty={}", i, price, qty);
            if !executions.is_empty() {
                println!("  Executed: {} trades", executions.len());
            }
        }
    }

    // Add some sell orders with matching
    for i in 5..10 {
        let price = 10050 - (i - 5) * 40; // Starts high, goes lower to match
        let qty = 200;

        let order = Order::new(i, price, qty, Side::Sell, OrderType::Limit);

        println!("Adding sell order id={} price={} qty={}", i, price, qty);

        if let Ok(executions) = book.add_order(order) {
            if !executions.is_empty() {
                println!("  Executed: {} trades", executions.len());
                for (j, exec) in executions.iter().enumerate() {
                    println!(
                        "    {}: order_id={} price={} qty={}",
                        j, exec.order_id, exec.price, exec.quantity
                    );
                }
            } else {
                println!("  No executions, order added to book");
            }
        }
    }

    // Print market depth
    let (bids, asks) = book.market_depth(10);

    println!("\nMarket Depth:");
    println!("Bids:");
    for (price, qty) in bids {
        println!("  {} @ {}", qty, price);
    }

    println!("Asks:");
    for (price, qty) in asks {
        println!("  {} @ {}", qty, price);
    }

    println!("\nOrderbook Summary:");
    println!("{}", book.summary());

    // Add a market order
    let market_order = Order::new(
        100,
        0, // price not used for market orders
        500,
        Side::Buy,
        OrderType::Market,
    );

    println!("\nAdding market buy order id=100 qty=500");

    if let Ok(executions) = book.add_order(market_order) {
        println!("  Executed: {} trades", executions.len());
        for (j, exec) in executions.iter().enumerate() {
            println!(
                "    {}: order_id={} price={} qty={}",
                j, exec.order_id, exec.price, exec.quantity
            );
        }
    }

    // Print final state
    println!("\nFinal Market Depth:");
    let (bids, asks) = book.market_depth(10);

    println!("Bids:");
    for (price, qty) in bids {
        println!("  {} @ {}", qty, price);
    }

    println!("Asks:");
    for (price, qty) in asks {
        println!("  {} @ {}", qty, price);
    }

    println!("\nFinal Orderbook Summary:");
    println!("{}", book.summary());
}
