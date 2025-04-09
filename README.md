# orderbook-rs

A low-latency, high-throughput orderbook implementation in Rust for trading systems and exchange infrastructure.

## Overview

This orderbook provides microsecond-level performance for order management and matching with a focus on:

- Memory efficiency
- CPU cache optimization
- Low-latency operations
- Price-time priority matching

## Performance

The implementation achieves:
- Sub-microsecond latency for core operations
- Throughput exceeding 1.5 million operations per second (M4 Pro 48gb)
- Support for order books with hundreds of thousands of resting orders
- Consistent performance under varied market conditions

## Key Features

- Price-time priority matching engine
- Limit and market order support
- Memory-efficient order representation
- Zero-copy design for critical paths

## Usage

```rust
use orderbook_rs::{OrderBook, Order, Side, OrderType};

// Create an orderbook
let mut book = OrderBook::new("FUEL-USD", 100_000);

// Add a buy order
let buy_order = Order::new(1, 9900, 10, Side::Buy, OrderType::Limit);
let result = book.add_order(buy_order).unwrap();

// Add a sell order
let sell_order = Order::new(2, 10000, 5, Side::Sell, OrderType::Limit);
let result = book.add_order(sell_order).unwrap();

// Check market depth
let (bids, asks) = book.market_depth(10);

// Get orderbook summary
println!("{}", book.summary());

// Cancel an order
book.cancel_order(1).unwrap();
```

## Implementation Details

The orderbook uses a Vec-based approach with direct indexing for O(1) price level access, rather than traditional tree-based structures. This design choice prioritizes:

- Contiguous memory layout for better cache locality
- Reduced pointer chasing and memory indirection
- Efficient traversal of price levels during matching

Price levels are constrained to a configurable range around a base price to enable this efficient indexing approach.

## Testing & Benchmarking

Run tests:
```
cargo test
```

Run benchmarks:
```
cargo run --release
```

The long-running benchmark simulates realistic market activity for sustained periods to assess performance stability and throughput.

## Requirements

- Rust 1.85 or higher
- SIMD supported on relevant CPUs

## License

GPL-3.0
