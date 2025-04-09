//! Order Book Implementation
//!
//! A microsecond-level performance order book implementation
//! focused on speed, efficiency, and low latency.

#![feature(portable_simd)]

pub mod benchmarks;
pub mod memory;
pub mod orderbook;
pub mod types;

pub use benchmarks::benchmark_orderbook;
pub use memory::{OrderPool, PriceLookupTable};
pub use orderbook::OrderBook;
pub use types::{Execution, Order, OrderType, Side};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Order, OrderType, Side};

    #[test]
    fn test_order_insertion() {
        let mut book = OrderBook::new("TEST", 1000);

        // Add a buy order
        let buy_order = Order::new(1, 9900, 10, Side::Buy, OrderType::Limit);
        let result = book.add_order(buy_order);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0); // No executions yet

        // Verify best bid
        assert_eq!(book.best_bid(), Some(9900));
        assert_eq!(book.best_ask(), None);

        // Add a sell order at higher price (no match)
        let sell_order = Order::new(2, 10000, 5, Side::Sell, OrderType::Limit);
        let result = book.add_order(sell_order);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0); // No executions

        // Verify best ask
        assert_eq!(book.best_bid(), Some(9900));
        assert_eq!(book.best_ask(), Some(10000));

        // Check spread
        assert_eq!(book.spread(), Some(100));

        // Check market depth
        let (bids, asks) = book.market_depth(10);
        assert_eq!(bids.len(), 1);
        assert_eq!(bids[0], (9900, 10)); // Price, quantity
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0], (10000, 5)); // Price, quantity

        // Check summary
        let summary = book.summary();
        assert_eq!(summary.order_count, 2);
        assert_eq!(summary.buy_levels, 1);
        assert_eq!(summary.sell_levels, 1);
    }

    #[test]
    fn test_order_matching() {
        let mut book = OrderBook::new("TEST", 1000);

        // Add a buy order
        let buy_order = Order::new(1, 9000, 10, Side::Buy, OrderType::Limit);
        let result = book.add_order(buy_order);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0); // No executions yet

        // Add a matching sell order
        let sell_order = Order::new(2, 9000, 5, Side::Sell, OrderType::Limit);
        let result = book.add_order(sell_order);
        assert!(result.is_ok());

        // Should have one execution
        let executions = result.unwrap();
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].order_id, 1); // First order ID
        assert_eq!(executions[0].price, 9000); // Match price
        assert_eq!(executions[0].quantity, 5); // Matched quantity

        // Check remaining quantity in the book
        let (bids, asks) = book.market_depth(10);
        assert_eq!(bids.len(), 1);
        assert_eq!(bids[0], (9000, 5)); // 5 quantity remaining at price 10000
        assert_eq!(asks.len(), 0); // Sell order fully matched

        // Check statistics
        let summary = book.summary();
        assert_eq!(summary.total_quantity_matched, 10);
    }

    #[test]
    fn test_price_time_priority() {
        let mut book = OrderBook::new("TEST", 1000);

        // Add buy orders at different prices
        let buy_order1 = Order::new(1, 9900, 10, Side::Buy, OrderType::Limit);
        let buy_order2 = Order::new(2, 9920, 10, Side::Buy, OrderType::Limit);
        book.add_order(buy_order1).unwrap();
        book.add_order(buy_order2).unwrap();

        // Add a matching sell order
        let sell_order = Order::new(3, 9900, 15, Side::Sell, OrderType::Limit);
        let result = book.add_order(sell_order);

        // Should have matched the higher price first
        let executions = result.unwrap();
        assert_eq!(executions.len(), 2);

        // First execution should be at higher price
        assert_eq!(executions[0].order_id, 2); // Higher price order
        assert_eq!(executions[0].price, 9920);
        assert_eq!(executions[0].quantity, 10);

        // Second execution should be at lower price
        assert_eq!(executions[1].order_id, 1); // Lower price order
        assert_eq!(executions[1].price, 9900);
        assert_eq!(executions[1].quantity, 5); // Only 5 left to match

        // Check remaining quantity in the book
        let (bids, _) = book.market_depth(10);
        assert_eq!(bids.len(), 1);
        assert_eq!(bids[0], (9900, 5)); // 5 quantity remaining at price 9900

        // Check statistics
        let summary = book.summary();
        assert_eq!(summary.total_quantity_matched, 15 * 2);
    }

    #[test]
    fn test_order_cancellation() {
        let mut book = OrderBook::new("TEST", 1000);

        // Add a buy order
        let buy_order = Order::new(1, 9900, 10, Side::Buy, OrderType::Limit);
        book.add_order(buy_order).unwrap();

        // Verify it's in the book
        assert_eq!(book.best_bid(), Some(9900));

        // Cancel the order
        let result = book.cancel_order(1);
        assert!(result.is_ok());

        // Check the book is empty
        assert_eq!(book.best_bid(), None);
        let (bids, _) = book.market_depth(10);
        assert_eq!(bids.len(), 0);

        // Check summary
        let summary = book.summary();
        assert_eq!(summary.order_count, 0);
    }

    #[test]
    fn test_market_order() {
        let mut book = OrderBook::new("TEST", 1000);

        // Add limit orders on the book
        let buy_order1 = Order::new(1, 9900, 10, Side::Buy, OrderType::Limit);
        let buy_order2 = Order::new(2, 9920, 10, Side::Buy, OrderType::Limit);
        book.add_order(buy_order1).unwrap();
        book.add_order(buy_order2).unwrap();

        // Add a market sell order
        let sell_order = Order::new(3, 0, 15, Side::Sell, OrderType::Market);
        let result = book.add_order(sell_order);

        // Should have matched both buy orders
        let executions = result.unwrap();
        assert_eq!(executions.len(), 2);

        // Should match the higher price first
        assert_eq!(executions[0].order_id, 2);
        assert_eq!(executions[0].price, 9920);
        assert_eq!(executions[0].quantity, 10);

        // Then the lower price
        assert_eq!(executions[1].order_id, 1);
        assert_eq!(executions[1].price, 9900);
        assert_eq!(executions[1].quantity, 5);

        // Check book is almost empty except for remaining 5 units
        let (bids, _) = book.market_depth(10);
        assert_eq!(bids.len(), 1);
        assert_eq!(bids[0], (9900, 5));

        // Check statistics
        let summary = book.summary();
        assert_eq!(summary.total_quantity_matched, 15);
    }

    #[test]
    fn test_price_boundary() {
        let mut book = OrderBook::new("TEST", 1000);

        // Test with prices at extremes of the allowed range

        // Base price is 10_000, so valid buy prices are below that
        // And valid sell prices are at or above that

        // Valid buy order
        let buy_order = Order::new(1, 9000, 10, Side::Buy, OrderType::Limit);
        let result = book.add_order(buy_order);
        assert!(result.is_ok());

        // Valid sell order
        let sell_order = Order::new(2, 11000, 10, Side::Sell, OrderType::Limit);
        let result = book.add_order(sell_order);
        assert!(result.is_ok());

        // Test buy order at exactly the base price (should be rejected)
        let buy_order = Order::new(3, 10000, 10, Side::Buy, OrderType::Limit);
        let result = book.add_order(buy_order);
        assert!(result.is_err());

        // Check that the right orders are in the book
        let (bids, asks) = book.market_depth(10);
        assert_eq!(bids.len(), 1);
        assert_eq!(bids[0], (9000, 10));
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0], (11000, 10));
    }

    #[test]
    fn test_partial_fills() {
        let mut book = OrderBook::new("TEST", 1000);

        // Add multiple buy orders at same price
        let buy_order1 = Order::new(1, 9900, 10, Side::Buy, OrderType::Limit);
        let buy_order2 = Order::new(2, 9900, 20, Side::Buy, OrderType::Limit);
        book.add_order(buy_order1).unwrap();
        book.add_order(buy_order2).unwrap();

        // Add a sell order that partially fills both
        let sell_order = Order::new(3, 9900, 15, Side::Sell, OrderType::Limit);
        let result = book.add_order(sell_order);

        // Should have executed against both orders in time priority
        let executions = result.unwrap();
        assert_eq!(executions.len(), 2);

        // First execution should be against first order (completely filled)
        assert_eq!(executions[0].order_id, 1);
        assert_eq!(executions[0].quantity, 10);

        // Second execution should be against second order (partially filled)
        assert_eq!(executions[1].order_id, 2);
        assert_eq!(executions[1].quantity, 5);

        // Check remaining quantity
        let (bids, _) = book.market_depth(10);
        assert_eq!(bids.len(), 1);
        assert_eq!(bids[0], (9900, 15)); // 15 remaining from second order

        // Check statistics
        let summary = book.summary();
        assert_eq!(summary.total_quantity_matched, 30);
    }

    #[test]
    fn test_multiple_price_levels() {
        let mut book = OrderBook::new("TEST", 1000);

        // Add buy orders at different prices
        let buy_order1 = Order::new(1, 9800, 10, Side::Buy, OrderType::Limit);
        let buy_order2 = Order::new(2, 9900, 20, Side::Buy, OrderType::Limit);
        let buy_order3 = Order::new(3, 9950, 30, Side::Buy, OrderType::Limit);
        book.add_order(buy_order1).unwrap();
        book.add_order(buy_order2).unwrap();
        book.add_order(buy_order3).unwrap();

        // Add sell orders at different prices
        let sell_order1 = Order::new(4, 10000, 15, Side::Sell, OrderType::Limit);
        let sell_order2 = Order::new(5, 10100, 25, Side::Sell, OrderType::Limit);
        let sell_order3 = Order::new(6, 10200, 35, Side::Sell, OrderType::Limit);
        book.add_order(sell_order1).unwrap();
        book.add_order(sell_order2).unwrap();
        book.add_order(sell_order3).unwrap();

        // Check market depth
        let (bids, asks) = book.market_depth(10);

        // Bids should be in descending price order
        assert_eq!(bids.len(), 3);
        assert_eq!(bids[0], (9950, 30));
        assert_eq!(bids[1], (9900, 20));
        assert_eq!(bids[2], (9800, 10));

        // Asks should be in ascending price order
        assert_eq!(asks.len(), 3);
        assert_eq!(asks[0], (10000, 15));
        assert_eq!(asks[1], (10100, 25));
        assert_eq!(asks[2], (10200, 35));

        // Check best bid/ask
        assert_eq!(book.best_bid(), Some(9950));
        assert_eq!(book.best_ask(), Some(10000));

        // Check spread
        assert_eq!(book.spread(), Some(50));
    }

    #[test]
    fn test_order_replacement() {
        let mut book = OrderBook::new("TEST", 1000);

        // Add an initial order
        let order = Order::new(1, 9900, 10, Side::Buy, OrderType::Limit);
        book.add_order(order).unwrap();

        // Try to add an order with same ID (should fail)
        let replacement = Order::new(1, 9950, 20, Side::Buy, OrderType::Limit);
        let result = book.add_order(replacement);
        assert!(result.is_err());

        // Cancel the original order
        book.cancel_order(1).unwrap();

        // Now we should be able to add a new order with the same ID
        let new_order = Order::new(1, 9950, 20, Side::Buy, OrderType::Limit);
        let result = book.add_order(new_order);
        assert!(result.is_ok());

        // Verify the new order is in the book
        assert_eq!(book.best_bid(), Some(9950));
        let (bids, _) = book.market_depth(10);
        assert_eq!(bids.len(), 1);
        assert_eq!(bids[0], (9950, 20));
    }

    #[test]
    fn test_large_volume() {
        let mut book = OrderBook::new("TEST", 10000);
        let mut order_count = 0;

        // Add a large number of orders
        for i in 0..100 {
            let price = 9500 + i * 5;
            let buy_order = Order::new(order_count, price, 10 + i, Side::Buy, OrderType::Limit);
            book.add_order(buy_order).unwrap();
            order_count += 1;

            let price = 10500 - i * 5;
            let sell_order = Order::new(order_count, price, 10 + i, Side::Sell, OrderType::Limit);
            book.add_order(sell_order).unwrap();
            order_count += 1;
        }

        // Check that we have the expected number of orders
        let summary = book.summary();
        assert_eq!(summary.order_count, 200);

        // Check market depth
        let (bids, asks) = book.market_depth(10);
        assert!(bids.len() > 0);
        assert!(asks.len() > 0);

        // Check best bid/ask
        assert!(book.best_bid().is_some());
        assert!(book.best_ask().is_some());

        // Add a market order to match against multiple levels
        let market_order = Order::new(order_count, 0, 500, Side::Buy, OrderType::Market);
        let result = book.add_order(market_order);
        assert!(result.is_ok());

        // Check that we matched some quantity
        let summary = book.summary();
        assert!(summary.total_quantity_matched > 0);
    }

    #[test]
    fn test_crossing_book() {
        let mut book = OrderBook::new("TEST", 1000);

        // Add a buy order
        let buy_order = Order::new(1, 9999, 10, Side::Buy, OrderType::Limit);
        book.add_order(buy_order).unwrap();

        // Add a sell order with lower price (should execute immediately)
        let sell_order = Order::new(2, 9900, 5, Side::Sell, OrderType::Limit);
        let result = book.add_order(sell_order);

        // Should have one execution
        let executions = result.unwrap();
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].order_id, 1); // Buy order ID
        assert_eq!(executions[0].price, 9999); // Should execute at the resting price
        assert_eq!(executions[0].quantity, 5); // Full quantity of the sell order

        // Check remaining quantity
        let (bids, asks) = book.market_depth(10);
        assert_eq!(bids.len(), 1);
        assert_eq!(bids[0], (9999, 5)); // 5 quantity remaining at price 10000
        assert_eq!(asks.len(), 0); // No asks remaining
    }
}
