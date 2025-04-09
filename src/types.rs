//! Core type definitions for the orderbook implementation

use std::time::Instant;

/// Order side enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

/// Order type enumeration - simplified to just Limit and Market
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}

/// Trade execution report
#[derive(Debug, Clone)]
pub struct Execution {
    pub order_id: u64,
    pub price: u64,
    pub quantity: u64,
    pub timestamp: u64,
    pub side: Side,
}

/// Represents an order in the system with minimal memory footprint
/// Designed for cache-friendly memory layout - 32 bytes total
#[derive(Clone)]
pub struct Order {
    pub order_id: u64,  // 8 bytes
    pub price: u64,     // 8 bytes
    pub quantity: u64,  // 8 bytes
    pub timestamp: u64, // 8 bytes
    // Using bit flags in a single byte to reduce size
    flags: u8, // 1 byte but padded to align
}

impl Order {
    #[inline]
    pub fn new(
        order_id: u64,
        price: u64,
        quantity: u64,
        side: Side,
        order_type: OrderType,
    ) -> Self {
        let mut flags = 0u8;

        // Set the side bit - 0 for buy, 1 for sell
        if side == Side::Sell {
            flags |= 1;
        }

        // Set the order type bit (using bit 1)
        // 0 for Limit, 1 for Market
        if order_type == OrderType::Market {
            flags |= 1 << 1;
        }

        Self {
            order_id,
            price,
            quantity,
            timestamp: precise_time_ns(), // Using a monotonic timestamp for ordering
            flags,
        }
    }

    #[inline]
    pub fn side(&self) -> Side {
        if self.flags & 1 == 0 {
            Side::Buy
        } else {
            Side::Sell
        }
    }

    #[inline]
    pub fn order_type(&self) -> OrderType {
        if (self.flags >> 1) & 1 == 0 {
            OrderType::Limit
        } else {
            OrderType::Market
        }
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.quantity > 0
    }
}

/// Function to get a precise timestamp in nanoseconds
#[inline]
pub fn precise_time_ns() -> u64 {
    let now = Instant::now();
    let duration = now.elapsed();
    (duration.as_secs() * 1_000_000_000) + duration.subsec_nanos() as u64
}

/// Represents a price level in the order book
/// Contains all orders at a specific price point
#[derive(Debug)]
pub struct PriceLevel {
    pub price: u64,
    pub total_quantity: u64,
    pub order_indices: Vec<usize>,
}

impl PriceLevel {
    pub fn new(price: u64, capacity: usize) -> Self {
        Self {
            price,
            total_quantity: 0,
            order_indices: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn add_order(&mut self, order_index: usize, quantity: u64) -> bool {
        self.order_indices.push(order_index);
        self.total_quantity += quantity;
        true
    }

    #[inline]
    pub fn remove_order(&mut self, order_index: usize, quantity: u64) -> bool {
        let position = self
            .order_indices
            .iter()
            .position(|&idx| idx == order_index);

        if let Some(pos) = position {
            // Remove order from list (swap and pop for O(1) removal)
            self.order_indices.swap_remove(pos);
            self.total_quantity -= quantity;
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.order_indices.is_empty()
    }

    #[inline]
    pub fn order_count(&self) -> usize {
        self.order_indices.len()
    }
}
