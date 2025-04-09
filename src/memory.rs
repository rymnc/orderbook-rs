//! Memory management utilities for high-performance allocation

use std::mem::MaybeUninit;
use std::simd::Simd;
use std::simd::cmp::SimdPartialEq;

use crate::types::Order;

/// Custom memory pool for orders to avoid heap allocations in the critical path
pub struct OrderPool {
    pool: Vec<MaybeUninit<Order>>,
    free_indices: Vec<usize>,
}

impl OrderPool {
    pub fn new(capacity: usize) -> Self {
        let mut pool = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            pool.push(MaybeUninit::uninit());
        }

        let mut free_indices = Vec::with_capacity(capacity);
        for i in (0..capacity).rev() {
            free_indices.push(i);
        }

        Self { pool, free_indices }
    }

    #[inline]
    pub fn allocate(&mut self, order: Order) -> Option<usize> {
        if let Some(index) = self.free_indices.pop() {
            self.pool[index] = MaybeUninit::new(order);
            Some(index)
        } else {
            None
        }
    }

    #[inline]
    pub fn deallocate(&mut self, index: usize) {
        self.free_indices.push(index);
    }

    #[inline]
    pub unsafe fn get(&self, index: usize) -> &Order {
        unsafe { self.pool[index].assume_init_ref() }
    }

    #[inline]
    pub unsafe fn get_mut(&mut self, index: usize) -> &mut Order {
        unsafe { self.pool[index].assume_init_mut() }
    }

    #[inline]
    pub fn available_capacity(&self) -> usize {
        self.free_indices.len()
    }

    #[inline]
    pub fn total_capacity(&self) -> usize {
        self.pool.len()
    }
}

/// SIMD-accelerated price lookup table
/// Provides O(1) access to price levels for fast matching
pub struct PriceLookupTable {
    prices: Vec<Simd<u64, 4>>,
    indices: Vec<Simd<u32, 4>>,
    size: usize,
}

impl PriceLookupTable {
    pub fn new(capacity: usize) -> Self {
        // Round up to the nearest multiple of 4 for SIMD alignment
        let vec_capacity = (capacity + 3) / 4;
        Self {
            prices: vec![Simd::splat(0); vec_capacity],
            indices: vec![Simd::splat(0); vec_capacity],
            size: 0,
        }
    }

    #[inline]
    pub fn insert(&mut self, price: u64, index: u32) {
        let simd_idx = self.size / 4;
        let lane = self.size % 4;

        if simd_idx >= self.prices.len() {
            // Resize if needed
            self.prices.push(Simd::splat(0));
            self.indices.push(Simd::splat(0));
        }

        // Update individual lane
        let mut price_vec = self.prices[simd_idx].to_array();
        let mut index_vec = self.indices[simd_idx].to_array();

        price_vec[lane] = price;
        index_vec[lane] = index;

        self.prices[simd_idx] = Simd::from_array(price_vec);
        self.indices[simd_idx] = Simd::from_array(index_vec);

        self.size += 1;
    }

    #[inline]
    pub fn find(&self, price: u64) -> Option<u32> {
        let search_val = Simd::splat(price);

        for i in 0..(self.size + 3) / 4 {
            let price_vec = self.prices[i];
            let index_vec = self.indices[i];

            // SIMD comparison - creates a mask where price matches
            let mask = price_vec.simd_eq(search_val);

            if !mask.any() {
                continue;
            }

            // Extract the matching lane index
            for lane in 0..4 {
                if mask.test(lane) && lane < self.size % 4 {
                    return Some(index_vec.as_array()[lane]);
                }
            }
        }

        None
    }

    #[inline]
    pub fn remove(&mut self, price: u64) -> bool {
        // Find the price
        let mut found = false;
        let mut idx = 0;
        let mut lane = 0;

        'outer: for i in 0..(self.size + 3) / 4 {
            let price_vec = self.prices[i];
            let search_val = Simd::splat(price);

            // SIMD comparison
            let mask = price_vec.simd_eq(search_val);

            if !mask.any() {
                continue;
            }

            // Find which lane matched
            for l in 0..4 {
                if mask.test(l) && l < self.size % 4 {
                    idx = i;
                    lane = l;
                    found = true;
                    break 'outer;
                }
            }
        }

        if found {
            // Remove by swapping with the last element
            let last_simd_idx = (self.size - 1) / 4;
            let last_lane = (self.size - 1) % 4;

            if idx == last_simd_idx && lane == last_lane {
                // It's the last element, just decrement size
                self.size -= 1;
                return true;
            }

            // Get the last element
            let last_price_vec = self.prices[last_simd_idx].to_array();
            let last_index_vec = self.indices[last_simd_idx].to_array();

            let last_price = last_price_vec[last_lane];
            let last_index = last_index_vec[last_lane];

            // Replace the removed element with the last one
            let mut price_vec = self.prices[idx].to_array();
            let mut index_vec = self.indices[idx].to_array();

            price_vec[lane] = last_price;
            index_vec[lane] = last_index;

            self.prices[idx] = Simd::from_array(price_vec);
            self.indices[idx] = Simd::from_array(index_vec);

            self.size -= 1;
            return true;
        }

        false
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.size
    }
}
