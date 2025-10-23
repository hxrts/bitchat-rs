//! Random number generation abstraction for deterministic simulation
//!
//! Provides clean trait for RNG that can be swapped between system randomness
//! and deterministic seeded randomness.

#![allow(dead_code)] // Public API for deterministic testing

use rand::{Rng, SeedableRng, rngs::StdRng};

/// Abstraction for random number generation
pub trait RandomSource: Send {
    /// Generate random u32
    fn gen_u32(&mut self) -> u32;
    
    /// Generate random u64
    fn gen_u64(&mut self) -> u64;
    
    /// Generate random f32 in range [0.0, 1.0)
    fn gen_f32(&mut self) -> f32;
    
    /// Generate random u32 in range
    fn gen_range_u32(&mut self, start: u32, end: u32) -> u32;
    
    /// Generate random u64 in range
    fn gen_range_u64(&mut self, start: u64, end: u64) -> u64;
}

/// System randomness using entropy-seeded RNG (Send-safe)
pub struct SystemRandom {
    rng: StdRng,
}

impl SystemRandom {
    pub fn new() -> Self {
        Self {
            rng: StdRng::from_entropy(),
        }
    }
}

impl Default for SystemRandom {
    fn default() -> Self {
        Self::new()
    }
}

impl RandomSource for SystemRandom {
    fn gen_u32(&mut self) -> u32 {
        self.rng.gen()
    }
    
    fn gen_u64(&mut self) -> u64 {
        self.rng.gen()
    }
    
    fn gen_f32(&mut self) -> f32 {
        self.rng.gen()
    }
    
    fn gen_range_u32(&mut self, start: u32, end: u32) -> u32 {
        self.rng.gen_range(start..end)
    }
    
    fn gen_range_u64(&mut self, start: u64, end: u64) -> u64 {
        self.rng.gen_range(start..end)
    }
}

/// Deterministic randomness using seeded RNG
pub struct SeededRandom {
    rng: StdRng,
    seed: u64,
}

impl SeededRandom {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
            seed,
        }
    }
    
    pub fn seed(&self) -> u64 {
        self.seed
    }
}

impl RandomSource for SeededRandom {
    fn gen_u32(&mut self) -> u32 {
        self.rng.gen()
    }
    
    fn gen_u64(&mut self) -> u64 {
        self.rng.gen()
    }
    
    fn gen_f32(&mut self) -> f32 {
        self.rng.gen()
    }
    
    fn gen_range_u32(&mut self, start: u32, end: u32) -> u32 {
        self.rng.gen_range(start..end)
    }
    
    fn gen_range_u64(&mut self, start: u64, end: u64) -> u64 {
        self.rng.gen_range(start..end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_random_generates() {
        let mut rng = SystemRandom::new();
        
        let _val1 = rng.gen_u32();
        let _val2 = rng.gen_f32();
        let val3 = rng.gen_range_u32(1, 100);
        
        assert!(val3 >= 1 && val3 < 100);
    }

    #[test]
    fn test_seeded_random_reproducible() {
        let mut rng1 = SeededRandom::new(42);
        let mut rng2 = SeededRandom::new(42);
        
        for _ in 0..100 {
            assert_eq!(rng1.gen_u32(), rng2.gen_u32());
        }
    }

    #[test]
    fn test_seeded_random_different_seeds() {
        let mut rng1 = SeededRandom::new(42);
        let mut rng2 = SeededRandom::new(43);
        
        let val1 = rng1.gen_u32();
        let val2 = rng2.gen_u32();
        
        assert_ne!(val1, val2);
    }

    #[test]
    fn test_seeded_random_range() {
        let mut rng = SeededRandom::new(12345);
        
        for _ in 0..100 {
            let val = rng.gen_range_u32(10, 20);
            assert!(val >= 10 && val < 20);
        }
    }

    #[test]
    fn test_seeded_random_stores_seed() {
        let rng = SeededRandom::new(99999);
        assert_eq!(rng.seed(), 99999);
    }
}

