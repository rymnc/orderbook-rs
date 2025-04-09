//! Core orderbook implementation using Vec instead of BTreeMap

#[cfg(feature = "perf")]
use std::time::{Duration, Instant};

use crate::memory::OrderPool;
use crate::types::{Execution, Order, OrderType, PriceLevel, Side, precise_time_ns};

/// Configuration constants
const PRICE_LEVELS: usize = 1024;
const DEFAULT_ORDERS_PER_LEVEL: usize = 1024;

/// High-performance orderbook implementation
/// Uses a Vec-based approach for O(1) price level access
pub struct OrderBook {
    symbol: String,
    order_pool: OrderPool,
    order_id_to_index: Vec<Option<usize>>, // Using a Vec for order_id -> index mapping
    max_order_id: u64,

    // Vec-based price levels instead of BTreeMap
    buy_levels: Vec<Option<PriceLevel>>,
    sell_levels: Vec<Option<PriceLevel>>,

    // Base price and tick size for price level indexing
    base_price: u64,
    tick_size: u64,

    // Cache best prices for O(1) lookup
    best_bid_idx: Option<usize>,
    best_ask_idx: Option<usize>,

    // Performance monitoring
    #[cfg(feature = "perf")]
    order_count: usize,
    #[cfg(feature = "perf")]
    last_insert_time: Duration,
    #[cfg(feature = "perf")]
    last_match_time: Duration,
    #[cfg(feature = "perf")]
    last_cancel_time: Duration,

    // Statistics counters
    total_orders_processed: u64,
    total_quantity_matched: u64,
}

impl OrderBook {
    /// Create a new orderbook with the given symbol and capacity
    pub fn new(symbol: &str, capacity: usize) -> Self {
        let mut buy_levels = Vec::with_capacity(PRICE_LEVELS);
        let mut sell_levels = Vec::with_capacity(PRICE_LEVELS);

        // Pre-allocate price level vectors
        for _ in 0..PRICE_LEVELS {
            buy_levels.push(None);
            sell_levels.push(None);
        }

        // Pre-allocate order ID lookup vector
        let mut order_id_to_index = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            order_id_to_index.push(None);
        }

        Self {
            symbol: symbol.to_string(),
            order_pool: OrderPool::new(capacity),
            order_id_to_index,
            max_order_id: 0,
            buy_levels,
            sell_levels,
            base_price: 10_000,
            tick_size: 1,
            best_bid_idx: None,
            best_ask_idx: None,
            #[cfg(feature = "perf")]
            order_count: 0,
            #[cfg(feature = "perf")]
            last_insert_time: Duration::default(),
            #[cfg(feature = "perf")]
            last_match_time: Duration::default(),
            #[cfg(feature = "perf")]
            last_cancel_time: Duration::default(),
            total_orders_processed: 0,
            total_quantity_matched: 0,
        }
    }

    /// Convert price to index for buy_levels
    #[inline]
    fn buy_price_to_idx(&self, price: u64) -> Option<usize> {
        // Ensure price is in valid range
        if price >= self.base_price {
            return None; // Price too high
        }

        let idx = ((self.base_price - price) / self.tick_size) as usize;
        if idx < PRICE_LEVELS {
            Some(idx)
        } else {
            None // Out of range
        }
    }

    /// Convert price to index for sell_levels
    #[inline]
    fn sell_price_to_idx(&self, price: u64) -> Option<usize> {
        // Ensure price is in valid range
        if price < self.base_price {
            return None; // Price too low
        }

        let idx = ((price - self.base_price) / self.tick_size) as usize;
        if idx < PRICE_LEVELS {
            Some(idx)
        } else {
            None // Out of range
        }
    }

    /// Convert buy_levels index to price
    #[inline]
    fn buy_idx_to_price(&self, idx: usize) -> u64 {
        self.base_price - (idx as u64 * self.tick_size)
    }

    /// Convert sell_levels index to price
    #[inline]
    fn sell_idx_to_price(&self, idx: usize) -> u64 {
        self.base_price + (idx as u64 * self.tick_size)
    }

    /// Find the index of the best bid (highest buy price)
    #[inline]
    fn find_best_bid_idx(&self) -> Option<usize> {
        // For buy, we want the lowest index (highest price)
        for i in 0..PRICE_LEVELS {
            if self.buy_levels[i].is_some() {
                return Some(i);
            }
        }
        None
    }

    /// Find the index of the best ask (lowest sell price)
    #[inline]
    fn find_best_ask_idx(&self) -> Option<usize> {
        // For sell, we want the lowest index (lowest price)
        for i in 0..PRICE_LEVELS {
            if self.sell_levels[i].is_some() {
                return Some(i);
            }
        }
        None
    }

    /// Add a new order to the book
    #[inline]
    pub fn add_order(&mut self, order: Order) -> Result<Vec<Execution>, String> {
        #[cfg(feature = "perf")]
        let start_time = Instant::now();

        // Ensure order ID is within our capacity
        if order.order_id >= self.order_id_to_index.len() as u64 {
            if order.order_id > self.max_order_id {
                self.max_order_id = order.order_id;

                // Expand order ID lookup vector if needed
                while self.order_id_to_index.len() <= order.order_id as usize {
                    self.order_id_to_index.push(None);
                }
            }
        }

        // Check if order ID already exists
        if self
            .order_id_to_index
            .get(order.order_id as usize)
            .map(|opt| opt.is_some())
            .unwrap_or(false)
        {
            return Err(format!("Order ID {} already exists", order.order_id));
        }

        self.total_orders_processed += 1;

        // Handle market orders immediately
        if order.order_type() == OrderType::Market {
            let executions = self.match_market_order(order);
            #[cfg(feature = "perf")]
            {
                self.last_match_time = start_time.elapsed();
            }
            return Ok(executions);
        }

        // For limit orders, try to match first
        let side = order.side();
        let price = order.price;
        let mut remaining_order = order.clone();
        let mut executions = Vec::with_capacity(10);

        // Try to match the order
        match side {
            Side::Buy => {
                if let Some(best_ask_idx) = self.best_ask_idx {
                    let best_ask = self.sell_idx_to_price(best_ask_idx);
                    if price >= best_ask {
                        executions = self.match_limit_order(&mut remaining_order);
                    }
                }
            }
            Side::Sell => {
                if let Some(best_bid_idx) = self.best_bid_idx {
                    let best_bid = self.buy_idx_to_price(best_bid_idx);
                    if price <= best_bid {
                        executions = self.match_limit_order(&mut remaining_order);
                    }
                }
            }
        }

        // If there's remaining quantity, add to the book
        if remaining_order.quantity > 0 {
            // Convert price to index
            let price_idx = match side {
                Side::Buy => self.buy_price_to_idx(price),
                Side::Sell => self.sell_price_to_idx(price),
            };

            // Check if price is within range
            if price_idx.is_none() {
                return Err(format!("Price {} is outside the allowed range", price));
            }

            let price_idx = price_idx.unwrap();

            // Allocate from the memory pool
            if let Some(index) = self.order_pool.allocate(remaining_order.clone()) {
                self.order_id_to_index[remaining_order.order_id as usize] = Some(index);

                // Add to the appropriate side of the book
                match side {
                    Side::Buy => {
                        // Get or create price level
                        let price_level = self.buy_levels[price_idx].get_or_insert_with(|| {
                            PriceLevel::new(price, DEFAULT_ORDERS_PER_LEVEL)
                        });

                        if !price_level.add_order(index, remaining_order.quantity) {
                            return Err("Price level full".to_string());
                        }

                        // Update best bid cache
                        if self.best_bid_idx.is_none() || price_idx < self.best_bid_idx.unwrap() {
                            self.best_bid_idx = Some(price_idx);
                        }
                    }
                    Side::Sell => {
                        // Get or create price level
                        let price_level = self.sell_levels[price_idx].get_or_insert_with(|| {
                            PriceLevel::new(price, DEFAULT_ORDERS_PER_LEVEL)
                        });

                        if !price_level.add_order(index, remaining_order.quantity) {
                            return Err("Price level full".to_string());
                        }

                        // Update best ask cache
                        if self.best_ask_idx.is_none() || price_idx < self.best_ask_idx.unwrap() {
                            self.best_ask_idx = Some(price_idx);
                        }
                    }
                }

                #[cfg(feature = "perf")]
                {
                    self.order_count += 1;
                }
            } else {
                return Err("Order pool full".to_string());
            }
        }

        // Update execution statistics
        for exec in &executions {
            self.total_quantity_matched += exec.quantity;
        }

        #[cfg(feature = "perf")]
        {
            self.last_insert_time = start_time.elapsed();
        }
        Ok(executions)
    }

    /// Cancel an existing order
    #[inline]
    pub fn cancel_order(&mut self, order_id: u64) -> Result<(), String> {
        #[cfg(feature = "perf")]
        let start_time = Instant::now();

        if order_id >= self.order_id_to_index.len() as u64 {
            return Err(format!("Order {} not found", order_id));
        }

        let index_opt = self.order_id_to_index[order_id as usize];

        if let Some(index) = index_opt {
            let order = unsafe { self.order_pool.get(index) };
            let side = order.side();
            let price = order.price;
            let quantity = order.quantity;

            // Remove from the appropriate side
            match side {
                Side::Buy => {
                    if let Some(price_idx) = self.buy_price_to_idx(price) {
                        if let Some(ref mut price_level) = self.buy_levels[price_idx] {
                            if !price_level.remove_order(index, quantity) {
                                return Err(format!("Failed to remove order from price level"));
                            }

                            // Remove empty price level and update best bid if needed
                            if price_level.is_empty() {
                                self.buy_levels[price_idx] = None;

                                // Update best bid cache
                                if Some(price_idx) == self.best_bid_idx {
                                    self.best_bid_idx = self.find_best_bid_idx();
                                }
                            }
                        } else {
                            return Err(format!("Price level {} not found", price));
                        }
                    } else {
                        return Err(format!("Price {} is outside the allowed range", price));
                    }
                }
                Side::Sell => {
                    if let Some(price_idx) = self.sell_price_to_idx(price) {
                        if let Some(ref mut price_level) = self.sell_levels[price_idx] {
                            if !price_level.remove_order(index, quantity) {
                                return Err(format!("Failed to remove order from price level"));
                            }

                            // Remove empty price level and update best ask if needed
                            if price_level.is_empty() {
                                self.sell_levels[price_idx] = None;

                                // Update best ask cache
                                if Some(price_idx) == self.best_ask_idx {
                                    self.best_ask_idx = self.find_best_ask_idx();
                                }
                            }
                        } else {
                            return Err(format!("Price level {} not found", price));
                        }
                    } else {
                        return Err(format!("Price {} is outside the allowed range", price));
                    }
                }
            }

            // Deallocate from the memory pool
            self.order_pool.deallocate(index);
            self.order_id_to_index[order_id as usize] = None;
            #[cfg(feature = "perf")]
            {
                self.order_count -= 1;
            }
        } else {
            return Err(format!("Order {} not found", order_id));
        }

        #[cfg(feature = "perf")]
        {
            self.last_cancel_time = start_time.elapsed();
        }
        Ok(())
    }

    /// Match a new limit order against the book
    #[inline]
    fn match_limit_order(&mut self, order: &mut Order) -> Vec<Execution> {
        #[cfg(feature = "perf")]
        let start_time = Instant::now();
        let mut executions = Vec::with_capacity(10);

        match order.side() {
            Side::Buy => {
                // Match against sells starting from the lowest price
                let mut current_idx = self.best_ask_idx;

                while let Some(idx) = current_idx {
                    if order.quantity == 0 {
                        break;
                    }

                    let price = self.sell_idx_to_price(idx);

                    // Check if the price is acceptable
                    if price > order.price {
                        break;
                    }

                    // Get a mutable reference to the price level
                    if let Some(ref mut level) = self.sell_levels[idx] {
                        // Process all orders at this level
                        let resting_indices = level.order_indices.clone();

                        for resting_idx in resting_indices {
                            if order.quantity == 0 {
                                break;
                            }

                            let resting_order = unsafe { self.order_pool.get_mut(resting_idx) };
                            let match_qty = std::cmp::min(resting_order.quantity, order.quantity);

                            // Update quantities
                            resting_order.quantity -= match_qty;
                            order.quantity -= match_qty;
                            level.total_quantity -= match_qty;

                            // Update matched quantity statistic
                            self.total_quantity_matched += match_qty;

                            // Create execution report
                            executions.push(Execution {
                                order_id: resting_order.order_id,
                                price,
                                quantity: match_qty,
                                timestamp: precise_time_ns(),
                                side: resting_order.side(),
                            });

                            // If resting order is fully matched, remove it
                            if resting_order.quantity == 0 {
                                level.order_indices.retain(|&idx| idx != resting_idx);
                                self.order_id_to_index[resting_order.order_id as usize] = None;
                                self.order_pool.deallocate(resting_idx);
                                #[cfg(feature = "perf")]
                                {
                                    self.order_count -= 1;
                                }
                            }
                        }

                        // If the level is now empty, remove it
                        if level.is_empty() {
                            self.sell_levels[idx] = None;

                            // Find the next price level
                            current_idx = None;
                            for i in (idx + 1)..PRICE_LEVELS {
                                if self.sell_levels[i].is_some() {
                                    current_idx = Some(i);
                                    break;
                                }
                            }

                            // Update best ask if needed
                            if Some(idx) == self.best_ask_idx {
                                self.best_ask_idx = current_idx;
                            }
                        }
                    } else {
                        // This price level should not be empty if we have an index
                        // Move to the next price level
                        current_idx = None;
                        for i in (idx + 1)..PRICE_LEVELS {
                            if self.sell_levels[i].is_some() {
                                current_idx = Some(i);
                                break;
                            }
                        }
                    }
                }
            }
            Side::Sell => {
                // Match against buys starting from the highest price
                let mut current_idx = self.best_bid_idx;

                while let Some(idx) = current_idx {
                    if order.quantity == 0 {
                        break;
                    }

                    let price = self.buy_idx_to_price(idx);

                    // Check if the price is acceptable
                    if price < order.price {
                        break;
                    }

                    // Get a mutable reference to the price level
                    if let Some(ref mut level) = self.buy_levels[idx] {
                        // Process all orders at this level
                        let resting_indices = level.order_indices.clone();

                        for resting_idx in resting_indices {
                            if order.quantity == 0 {
                                break;
                            }

                            let resting_order = unsafe { self.order_pool.get_mut(resting_idx) };
                            let match_qty = std::cmp::min(resting_order.quantity, order.quantity);

                            // Update quantities
                            resting_order.quantity -= match_qty;
                            order.quantity -= match_qty;
                            level.total_quantity -= match_qty;

                            // Update matched quantity statistic
                            self.total_quantity_matched += match_qty;

                            // Create execution report
                            executions.push(Execution {
                                order_id: resting_order.order_id,
                                price,
                                quantity: match_qty,
                                timestamp: precise_time_ns(),
                                side: resting_order.side(),
                            });

                            // If resting order is fully matched, remove it
                            if resting_order.quantity == 0 {
                                level.order_indices.retain(|&idx| idx != resting_idx);
                                self.order_id_to_index[resting_order.order_id as usize] = None;
                                self.order_pool.deallocate(resting_idx);
                                #[cfg(feature = "perf")]
                                {
                                    self.order_count -= 1;
                                }
                            }
                        }

                        // If the level is now empty, remove it
                        if level.is_empty() {
                            self.buy_levels[idx] = None;

                            // Find the next price level
                            current_idx = None;
                            for i in (idx + 1)..PRICE_LEVELS {
                                if self.buy_levels[i].is_some() {
                                    current_idx = Some(i);
                                    break;
                                }
                            }

                            // Update best bid if needed
                            if Some(idx) == self.best_bid_idx {
                                self.best_bid_idx = current_idx;
                            }
                        }
                    } else {
                        // This price level should not be empty if we have an index
                        // Move to the next price level
                        current_idx = None;
                        for i in (idx + 1)..PRICE_LEVELS {
                            if self.buy_levels[i].is_some() {
                                current_idx = Some(i);
                                break;
                            }
                        }
                    }
                }
            }
        }

        #[cfg(feature = "perf")]
        {
            self.last_match_time = start_time.elapsed();
        }
        executions
    }

    /// Match a new market order against the book
    #[inline]
    fn match_market_order(&mut self, mut order: Order) -> Vec<Execution> {
        // For market orders, we don't care about price constraints
        // We just match against the best available prices until filled or liquidity exhausted
        match order.side() {
            Side::Buy => {
                // Match against sells starting from the lowest price
                let mut executions = Vec::with_capacity(10);
                let mut current_idx = self.best_ask_idx;

                while let Some(idx) = current_idx {
                    if order.quantity == 0 {
                        break;
                    }

                    let price = self.sell_idx_to_price(idx);

                    // Get a mutable reference to the price level
                    if let Some(ref mut level) = self.sell_levels[idx] {
                        // Process all orders at this level
                        let resting_indices = level.order_indices.clone();

                        for resting_idx in resting_indices {
                            if order.quantity == 0 {
                                break;
                            }

                            let resting_order = unsafe { self.order_pool.get_mut(resting_idx) };
                            let match_qty = std::cmp::min(resting_order.quantity, order.quantity);

                            // Update quantities
                            resting_order.quantity -= match_qty;
                            order.quantity -= match_qty;
                            level.total_quantity -= match_qty;
                            self.total_quantity_matched += match_qty;

                            // Create execution report
                            executions.push(Execution {
                                order_id: resting_order.order_id,
                                price,
                                quantity: match_qty,
                                timestamp: precise_time_ns(),
                                side: resting_order.side(),
                            });

                            // If resting order is fully matched, remove it
                            if resting_order.quantity == 0 {
                                level.order_indices.retain(|&idx| idx != resting_idx);
                                self.order_id_to_index[resting_order.order_id as usize] = None;
                                self.order_pool.deallocate(resting_idx);
                                #[cfg(feature = "perf")]
                                {
                                    self.order_count -= 1;
                                }
                            }
                        }

                        // If the level is now empty, remove it
                        if level.is_empty() {
                            self.sell_levels[idx] = None;

                            // Find the next price level
                            current_idx = None;
                            for i in (idx + 1)..PRICE_LEVELS {
                                if self.sell_levels[i].is_some() {
                                    current_idx = Some(i);
                                    break;
                                }
                            }

                            // Update best ask if needed
                            if Some(idx) == self.best_ask_idx {
                                self.best_ask_idx = current_idx;
                            }
                        }
                    } else {
                        // Move to the next price level
                        current_idx = None;
                        for i in (idx + 1)..PRICE_LEVELS {
                            if self.sell_levels[i].is_some() {
                                current_idx = Some(i);
                                break;
                            }
                        }
                    }
                }

                executions
            }
            Side::Sell => {
                // Match against buys starting from the highest price
                let mut executions = Vec::with_capacity(10);
                let mut current_idx = self.best_bid_idx;

                while let Some(idx) = current_idx {
                    if order.quantity == 0 {
                        break;
                    }

                    let price = self.buy_idx_to_price(idx);

                    // Get a mutable reference to the price level
                    if let Some(ref mut level) = self.buy_levels[idx] {
                        // Process all orders at this level
                        let resting_indices = level.order_indices.clone();

                        for resting_idx in resting_indices {
                            if order.quantity == 0 {
                                break;
                            }

                            let resting_order = unsafe { self.order_pool.get_mut(resting_idx) };
                            let match_qty = std::cmp::min(resting_order.quantity, order.quantity);

                            // Update quantities
                            resting_order.quantity -= match_qty;
                            order.quantity -= match_qty;
                            level.total_quantity -= match_qty;
                            self.total_quantity_matched += match_qty;

                            // Create execution report
                            executions.push(Execution {
                                order_id: resting_order.order_id,
                                price,
                                quantity: match_qty,
                                timestamp: precise_time_ns(),
                                side: resting_order.side(),
                            });

                            // If resting order is fully matched, remove it
                            if resting_order.quantity == 0 {
                                level.order_indices.retain(|&idx| idx != resting_idx);
                                self.order_id_to_index[resting_order.order_id as usize] = None;
                                self.order_pool.deallocate(resting_idx);
                                #[cfg(feature = "perf")]
                                {
                                    self.order_count -= 1;
                                }
                            }
                        }

                        // If the level is now empty, remove it
                        if level.is_empty() {
                            self.buy_levels[idx] = None;

                            // Find the next price level
                            current_idx = None;
                            for i in (idx + 1)..PRICE_LEVELS {
                                if self.buy_levels[i].is_some() {
                                    current_idx = Some(i);
                                    break;
                                }
                            }

                            // Update best bid if needed
                            if Some(idx) == self.best_bid_idx {
                                self.best_bid_idx = current_idx;
                            }
                        }
                    } else {
                        // Move to the next price level
                        current_idx = None;
                        for i in (idx + 1)..PRICE_LEVELS {
                            if self.buy_levels[i].is_some() {
                                current_idx = Some(i);
                                break;
                            }
                        }
                    }
                }

                executions
            }
        }
    }

    /// Get a snapshot of market depth
    pub fn market_depth(&self, levels: usize) -> (Vec<(u64, u64)>, Vec<(u64, u64)>) {
        let mut bids = Vec::with_capacity(levels);
        let mut asks = Vec::with_capacity(levels);

        // Get bid depth (highest to lowest)
        let mut count = 0;
        // For buys, we want to scan from lowest index (highest price) upward
        for idx in 0..PRICE_LEVELS {
            if count >= levels {
                break;
            }

            if let Some(ref level) = self.buy_levels[idx] {
                bids.push((self.buy_idx_to_price(idx), level.total_quantity));
                count += 1;
            }
        }

        // Get ask depth (lowest to highest)
        let mut count = 0;
        // For sells, we want to scan from lowest index (lowest price) upward
        for idx in 0..PRICE_LEVELS {
            if count >= levels {
                break;
            }

            if let Some(ref level) = self.sell_levels[idx] {
                asks.push((self.sell_idx_to_price(idx), level.total_quantity));
                count += 1;
            }
        }

        (bids, asks)
    }

    /// Get performance statistics
    #[cfg(feature = "perf")]
    pub fn performance_stats(&self) -> (Duration, Duration, Duration, usize) {
        (
            self.last_insert_time,
            self.last_match_time,
            self.last_cancel_time,
            self.order_count,
        )
    }

    /// Get the symbol for this orderbook
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// Get the best bid price
    pub fn best_bid(&self) -> Option<u64> {
        self.best_bid_idx.map(|idx| self.buy_idx_to_price(idx))
    }

    /// Get the best ask price
    pub fn best_ask(&self) -> Option<u64> {
        self.best_ask_idx.map(|idx| self.sell_idx_to_price(idx))
    }

    /// Get the mid price
    pub fn mid_price(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some((bid as f64 + ask as f64) / 2.0),
            _ => None,
        }
    }

    /// Get the spread
    pub fn spread(&self) -> Option<u64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Check if this orderbook is crossed (invalid state)
    pub fn is_crossed(&self) -> bool {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => bid >= ask,
            _ => false,
        }
    }

    /// Get a summary of the current orderbook state
    pub fn summary(&self) -> OrderBookSummary {
        let mut buy_level_count = 0;
        let mut sell_level_count = 0;

        for level in &self.buy_levels {
            if level.is_some() {
                buy_level_count += 1;
            }
        }

        for level in &self.sell_levels {
            if level.is_some() {
                sell_level_count += 1;
            }
        }

        OrderBookSummary {
            symbol: self.symbol.clone(),
            best_bid: self.best_bid(),
            best_ask: self.best_ask(),
            buy_levels: buy_level_count,
            sell_levels: sell_level_count,
            #[cfg(feature = "perf")]
            order_count: self.order_count,
            total_orders_processed: self.total_orders_processed,
            total_quantity_matched: self.total_quantity_matched,
            #[cfg(feature = "perf")]
            last_insert_time_ns: self.last_insert_time.as_nanos() as u64,
            #[cfg(feature = "perf")]
            last_match_time_ns: self.last_match_time.as_nanos() as u64,
            #[cfg(feature = "perf")]
            last_cancel_time_ns: self.last_cancel_time.as_nanos() as u64,
        }
    }
}

/// A summary of the orderbook state
#[derive(Debug, Clone)]
pub struct OrderBookSummary {
    pub symbol: String,
    pub best_bid: Option<u64>,
    pub best_ask: Option<u64>,
    pub buy_levels: usize,
    pub sell_levels: usize,
    #[cfg(feature = "perf")]
    pub order_count: usize,
    pub total_orders_processed: u64,
    pub total_quantity_matched: u64,
    #[cfg(feature = "perf")]
    pub last_insert_time_ns: u64,
    #[cfg(feature = "perf")]
    pub last_match_time_ns: u64,
    #[cfg(feature = "perf")]
    pub last_cancel_time_ns: u64,
}

impl std::fmt::Display for OrderBookSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "OrderBook Summary for {}", self.symbol)?;
        writeln!(f, "----------------------------")?;

        if let Some(bid) = self.best_bid {
            writeln!(f, "Best Bid: {}", bid)?;
        } else {
            writeln!(f, "Best Bid: None")?;
        }

        if let Some(ask) = self.best_ask {
            writeln!(f, "Best Ask: {}", ask)?;
        } else {
            writeln!(f, "Best Ask: None")?;
        }

        writeln!(f, "Buy Levels: {}", self.buy_levels)?;
        writeln!(f, "Sell Levels: {}", self.sell_levels)?;
        writeln!(f, "Processed Orders: {}", self.total_orders_processed)?;
        writeln!(f, "Matched Quantity: {}", self.total_quantity_matched)?;
        #[cfg(feature = "perf")]
        {
            writeln!(f, "Total Orders: {}", self.order_count)?;
            writeln!(f, "Last Insert Time: {} ns", self.last_insert_time_ns)?;
            writeln!(f, "Last Match Time: {} ns", self.last_match_time_ns)?;
            writeln!(f, "Last Cancel Time: {} ns", self.last_cancel_time_ns)?;
        }

        Ok(())
    }
}
