/// Minimal seeded RNG based on xorshift64.
///
/// Not cryptographic, but uniform and fast enough for procedural generation.
/// The same seed always produces the same sequence.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Ensure the state is never zero (xorshift is stuck at 0 forever).
        Rng(if seed == 0 { 0xDEAD_BEEF_CAFE_1234 } else { seed })
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Returns a uniform float in [0.0, 1.0).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 32) as f32 / (u32::MAX as f32 + 1.0)
    }

    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}
