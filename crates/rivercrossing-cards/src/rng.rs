// SPDX-License-Identifier: GPL-3.0-only

//! The seeded random source: a `CPython`-compatible `Mersenne Twister`.
//!
//! The state and the seeding match `CPython`'s `random.Random(seed)` for a non-negative
//! integer seed, so a shuffle port reproduces the oracle exactly.

/// The number of 32-bit words in the `Mersenne Twister` state.
const MT_SIZE: usize = 624;
/// The middle-word distance of the twist recurrence.
const MIDDLE_WORD: usize = 397;
/// The twist matrix constant.
const MATRIX_A: u32 = 0x9908_b0df;
/// The most significant bit mask.
const UPPER_MASK: u32 = 0x8000_0000;
/// The remaining 31 bits mask.
const LOWER_MASK: u32 = 0x7fff_ffff;

/// A `Mersenne Twister` generator that reproduces `CPython`'s `random.Random(seed)`.
#[derive(Debug)]
pub(crate) struct CpRandom {
    mt: [u32; MT_SIZE],
    index: usize,
}

impl CpRandom {
    /// Return a generator seeded the way `CPython` seeds `random.Random(seed)`.
    pub(crate) fn new(seed: u64) -> Self {
        let [b0, b1, b2, b3, b4, b5, b6, b7] = seed.to_le_bytes();
        let key = [
            u32::from_le_bytes([b0, b1, b2, b3]),
            u32::from_le_bytes([b4, b5, b6, b7]),
        ];
        let bits = u64::BITS - seed.leading_zeros();
        let key_used = bits.div_ceil(32).max(1);
        let mut random = Self {
            mt: [0; MT_SIZE],
            index: MT_SIZE,
        };
        random.init_by_array(key, key_used);
        random
    }

    /// Return the top `bits` bits of the next 32-bit word, for `bits` in `1..=32`.
    pub(crate) fn getrandbits(&mut self, bits: u32) -> u32 {
        self.genrand_uint32() >> (32 - bits)
    }

    /// Return a value in `0..bound`, by rejection of the top bits that overflow the bound.
    pub(crate) fn randbelow(&mut self, bound: usize) -> usize {
        if bound <= 1 {
            return 0;
        }
        let bits = usize::BITS - bound.leading_zeros();
        loop {
            let value = self.getrandbits(bits) as usize;
            if value < bound {
                return value;
            }
        }
    }

    /// Shuffle the items as `CPython`'s `random.Random(seed).shuffle` does.
    pub(crate) fn shuffle<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            let target = self.randbelow(index + 1);
            items.swap(index, target);
        }
    }

    /// Fill the state from one word, the reference `init_genrand` step.
    #[expect(
        clippy::indexing_slicing,
        clippy::cast_possible_truncation,
        reason = "the loop counter stays below the 624-word state size, so both uses are in range"
    )]
    fn init_genrand(&mut self, seed: u32) {
        self.mt[0] = seed;
        for index in 1..MT_SIZE {
            let previous = self.mt[index - 1];
            let mixed = (previous ^ (previous >> 30)).wrapping_mul(1_812_433_253);
            self.mt[index] = mixed.wrapping_add(index as u32);
        }
        self.index = MT_SIZE;
    }

    /// Fill the state from a key array, the reference `init_by_array` step.
    #[expect(
        clippy::indexing_slicing,
        clippy::cast_possible_truncation,
        reason = "the counters stay below the 624-word state and the two-word key, so both uses are in range"
    )]
    fn init_by_array(&mut self, key: [u32; 2], key_used: u32) {
        self.init_genrand(19_650_218);
        let key_len = key_used as usize;
        let mut index = 1;
        let mut key_index = 0;
        for _ in 0..MT_SIZE.max(key_len) {
            let previous = self.mt[index - 1];
            let mixed = (previous ^ (previous >> 30)).wrapping_mul(1_664_525);
            let value = self.mt[index] ^ mixed;
            self.mt[index] = value
                .wrapping_add(key[key_index])
                .wrapping_add(key_index as u32);
            index += 1;
            key_index += 1;
            if index >= MT_SIZE {
                self.mt[0] = self.mt[MT_SIZE - 1];
                index = 1;
            }
            if key_index >= key_len {
                key_index = 0;
            }
        }
        for _ in 0..MT_SIZE - 1 {
            let previous = self.mt[index - 1];
            let mixed = (previous ^ (previous >> 30)).wrapping_mul(1_566_083_941);
            let value = self.mt[index] ^ mixed;
            self.mt[index] = value.wrapping_sub(index as u32);
            index += 1;
            if index >= MT_SIZE {
                self.mt[0] = self.mt[MT_SIZE - 1];
                index = 1;
            }
        }
        self.mt[0] = 0x8000_0000;
    }

    /// Regenerate the whole state, the reference twist step.
    #[expect(
        clippy::indexing_slicing,
        reason = "every index is reduced modulo the 624-word state size before use"
    )]
    fn twist(&mut self) {
        for index in 0..MT_SIZE {
            let next = self.mt[(index + 1) % MT_SIZE];
            let y = (self.mt[index] & UPPER_MASK) | (next & LOWER_MASK);
            let matrix = if y & 1 == 0 { 0 } else { MATRIX_A };
            let shifted = self.mt[(index + MIDDLE_WORD) % MT_SIZE];
            self.mt[index] = shifted ^ (y >> 1) ^ matrix;
        }
        self.index = 0;
    }

    /// Return the next tempered 32-bit word.
    #[expect(
        clippy::indexing_slicing,
        reason = "the index is reset by the twist before it can pass the state size"
    )]
    fn genrand_uint32(&mut self) -> u32 {
        if self.index >= MT_SIZE {
            self.twist();
        }
        let mut value = self.mt[self.index];
        self.index += 1;
        value ^= value >> 11;
        value ^= (value << 7) & 0x9d2c_5680;
        value ^= (value << 15) & 0xefc6_0000;
        value ^= value >> 18;
        value
    }
}

#[cfg(test)]
mod tests;
