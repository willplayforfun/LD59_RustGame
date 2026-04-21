// Pure star data and math.  No pixel manipulation — see star_rendering.rs.

use crate::rng::Rng;

// ─── Procedural star data ─────────────────────────────────────────────────────

/// Derives an independent seed for star `index` from the global world seed.
///
/// Uses a hash (Weyl sequence + SplitMix64 avalanche) so star N can be
/// generated without first generating stars 0..N-1.
pub fn star_seed(global_seed: u64, index: usize) -> u64 {
    let mut x = global_seed.wrapping_add((index as u64).wrapping_mul(0x9e3779b97f4a7c15));
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

/// A planet orbiting a star, described in the parameters the player will try
/// to match.
#[derive(Clone, Debug)]
pub struct Planet {
    /// Relative mass (Jupiter = 1.0).  Controls wobble amplitude.
    pub mass: f32,
    /// Orbital period in seconds (game time).
    pub period: f32,
    /// Orbit eccentricity: 0.0 = circular, ~0.9 = very elongated.
    pub eccentricity: f32,
    /// Unit vector giving the wobble axis projected onto the observation plane.
    pub direction: (f32, f32),
}

/// All observable and hidden properties of a single star.
#[derive(Clone, Debug)]
pub struct StarData {
    /// Normalised screen position: both components in [0.0, 1.0].
    pub position: (f32, f32),
    /// Blackbody temperature in Kelvin.  Determines colour.
    pub temperature: f32,
    /// Nominal brightness (same scale as `generate_starfield`).
    pub brightness: f32,
    /// The true planetary system — what the player must discover.
    pub planets: Vec<Planet>,
}

/// Generates all properties for star `index` deterministically from
/// `global_seed`.  `difficulty` (1–6) controls how many planets may appear:
/// low difficulties almost always yield 1; high difficulties can yield 2–3.
pub fn generate_star_data(global_seed: u64, index: usize, difficulty: u8) -> StarData {
    let mut rng = Rng::new(star_seed(global_seed, index));

    let position    = (rng.next_f32(), rng.next_f32());
    let temperature = rng.range_f32(3_000.0, 20_000.0);
    let brightness  = f32::exp(rng.range_f32(-2.0, 1.5));

    // Roll determines how many *extra* planets beyond the guaranteed 1.
    // Thresholds (out of 100): [p(+0), p(+0 or +1)] — anything above → +2.
    let roll = rng.next_u64() % 100;
    let extra = match difficulty {
        1 | 2         => if roll < 93 { 0 } else { 1 },
        3 | 4         => if roll < 50 { 0 } else { 1 },
        _ /* 5–6+ */ => if roll < 20 { 0 } else if roll < 75 { 1 } else { 2 },
    };
    let planet_count = 1 + extra;
    let planets = (0..planet_count)
        .map(|_| {
            let angle = rng.next_f32() * std::f32::consts::TAU;
            Planet {
                mass:         rng.range_f32(0.3, 5.0),
                period:       rng.range_f32(5.0, 40.0),
                eccentricity: rng.range_f32(0.0, 0.7),
                direction:    (1.0, 0.0)//(angle.cos(), angle.sin()),
            }
        })
        .collect();

    StarData { position, temperature, brightness, planets }
}

// ─── Orbital wobble ──────────────────────────────────────────────────────────

/// Returns the `(x, y)` sub-pixel offset of a star being tugged by a planet.
///
/// The displacement follows a Lorentzian pulse — `1 / (1 + (phase/τ)²)` —
/// which matches the shape of a 1/r² gravitational force integrated over a
/// flyby.
///
/// `time`      — current time in seconds.
/// `period`    — orbital period in seconds.
/// `amplitude` — peak displacement in pixels at closest approach.
/// `sharpness` — fraction of the period over which the tug is felt (0.05–0.2).
/// `direction` — unit vector `(x, y)` giving the wobble axis.
pub fn planet_wobble_offset(
    time: f32,
    period: f32,
    amplitude: f32,
    sharpness: f32,
    direction: (f32, f32),
) -> (f32, f32) {
    let phase  = (time / period).fract() - 0.5;
    let tau    = sharpness * 0.5;
    let pulse  = 1.0 / (1.0 + (phase / tau).powi(2));
    let offset = amplitude * pulse;
    (offset * direction.0, offset * direction.1)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_seed_is_deterministic() {
        assert_eq!(star_seed(42, 7), star_seed(42, 7));
    }

    #[test]
    fn star_seed_differs_by_index() {
        assert_ne!(star_seed(42, 0), star_seed(42, 1));
    }

    #[test]
    fn star_seed_differs_by_global_seed() {
        assert_ne!(star_seed(1, 0), star_seed(2, 0));
    }

    #[test]
    fn generate_star_data_is_deterministic() {
        let a = generate_star_data(99, 5, 3);
        let b = generate_star_data(99, 5, 3);
        assert_eq!(a.temperature, b.temperature);
        assert_eq!(a.position,    b.position);
        assert_eq!(a.planets.len(), b.planets.len());
    }

    #[test]
    fn generate_star_data_differs_by_index() {
        let a = generate_star_data(99, 0, 3);
        let b = generate_star_data(99, 1, 3);
        assert_ne!(a.temperature, b.temperature);
    }

    #[test]
    fn planet_count_low_difficulty() {
        for i in 0..100 {
            let n = generate_star_data(12345, i, 1).planets.len();
            assert!(n >= 1 && n <= 2, "difficulty 1 yielded {n} planets");
        }
    }

    #[test]
    fn planet_count_high_difficulty() {
        for i in 0..100 {
            let n = generate_star_data(12345, i, 6).planets.len();
            assert!(n >= 1 && n <= 3, "difficulty 6 yielded {n} planets");
        }
    }

    #[test]
    fn star_positions_are_normalised() {
        for i in 0..20 {
            let d = generate_star_data(777, i, 1);
            assert!((0.0..=1.0).contains(&d.position.0));
            assert!((0.0..=1.0).contains(&d.position.1));
        }
    }
}
