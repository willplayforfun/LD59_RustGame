// Predicts what an observer would measure for a planetary system at time T.
//
// All output values are dimensionless gameplay numbers, scaled to stay
// comfortably in the 0–1000 range for typical planet parameters.
//
// Viewing geometry is controlled by `inclination`:
//   1.0 (EDGE_ON)  — transits visible, wobble is 1-D, full radial velocity.
//   0.0 (FACE_ON)  — no transits, circular sky-plane wobble, no redshift.
// The default everywhere is EDGE_ON; the parameter is plumbed through so it
// can be varied per-star later without changing the API.

use std::f32::consts::TAU;
use crate::star::Planet;

// ─── Viewing geometry presets ─────────────────────────────────────────────────

pub const EDGE_ON:  f32 = 1.0;
#[allow(dead_code)]
pub const FACE_ON:  f32 = 0.0;

// ─── Scaling constants ────────────────────────────────────────────────────────
// Adjust these to keep gameplay readouts in a comfortable 0–1000 range.
// They intentionally have no physical units.

/// Brightness with no planets and no transit in progress.
const BRIGHTNESS_BASE: f32 = 500.0;

/// Brightness drop per Jupiter-mass during a transit.
const TRANSIT_DEPTH_PER_MASS: f32 = 50.0;

/// Fraction of the orbital period spent in transit.
const TRANSIT_DURATION_FRAC: f32 = 0.08;

/// Redshift amplitude per Jupiter-mass.
const REDSHIFT_PER_MASS: f32 = 20.0;

/// Astrometric offset amplitude (graph units) per Jupiter-mass.
const POSITION_PER_MASS: f32 = 30.0;

/// Sub-pixel wobble amplitude (pixels) per Jupiter-mass for the star texture.
const PIXEL_WOBBLE_PER_MASS: f32 = 0.3;

// ─── Output type ─────────────────────────────────────────────────────────────

/// All three observable signals at a single instant.
#[derive(Clone, Copy, Debug)]
pub struct Observations {
    /// Apparent brightness. Nominal ≈ BRIGHTNESS_BASE; dips during transits.
    pub brightness: f32,
    /// Doppler shift proxy. Positive = receding, negative = approaching.
    pub redshift: f32,
    /// Sky-plane astrometric offset (graph units). For edge-on systems the Y
    /// component is near zero; graphs currently display only X.
    pub position: (f32, f32),
}

// ─── Orbital phase ────────────────────────────────────────────────────────────

/// Converts `time` to an orbital angle in radians using a first-order Kepler
/// correction for eccentricity.
///
/// At e = 0 this is simply `2π × fract(t / period)`. Higher eccentricity
/// makes the planet sprint near periapsis and linger near apoapsis, which
/// sharpens the peaks in the redshift and position curves.
fn orbital_angle(time: f32, period: f32, eccentricity: f32) -> f32 {
    let mean = TAU * (time / period).fract();
    mean + eccentricity * mean.sin()    // first-order eccentric anomaly
}

// ─── Transit brightness dip ───────────────────────────────────────────────────

/// Returns a negative brightness delta when a transit is in progress.
///
/// A **box dip** drops brightness to a flat reduced level for the whole
/// transit duration, then restores it instantly — the light curve is a
/// rectangular notch. Real transits have rounded edges (limb darkening),
/// but the box shape is easy to reason about and clearly encodes depth and
/// period. Swap this function out to experiment with other shapes.
///
/// The transit midpoint is at orbital angle π/2 (sin = 1, planet between
/// star and observer).
fn box_dip(angle: f32, depth: f32, duration_frac: f32) -> f32 {
    let half_width = TAU * duration_frac * 0.5;
    let delta      = (angle - TAU * 0.25).rem_euclid(TAU);
    let delta      = delta.min(TAU - delta);    // shortest arc to midpoint
    if delta < half_width { -depth } else { 0.0 }
}

// ─── Per-planet contribution ──────────────────────────────────────────────────

fn planet_contribution(planet: &Planet, time: f32, inclination: f32) -> Observations {
    let angle = orbital_angle(time, planet.period, planet.eccentricity);
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    // Vector perpendicular to the planet's wobble direction (in screen space).
    let perp = (-planet.direction.1, planet.direction.0);

    // Astrometric offset:
    //   cos component → along the wobble axis   (always visible)
    //   sin component → along the perp axis     (suppressed at edge-on)
    let amp   = planet.mass * POSITION_PER_MASS;
    let pos_x = planet.direction.0 * cos_a * amp
              + perp.0              * sin_a * amp * (1.0 - inclination);
    let pos_y = planet.direction.1 * cos_a * amp
              + perp.1              * sin_a * amp * (1.0 - inclination);

    // Radial velocity: the sin component, fully visible when edge-on.
    let redshift = sin_a * planet.mass * REDSHIFT_PER_MASS * inclination;

    // Transit dip: no transit for face-on systems.
    let depth   = planet.mass * TRANSIT_DEPTH_PER_MASS * inclination;
    let b_delta = box_dip(angle, depth, TRANSIT_DURATION_FRAC);

    Observations { brightness: b_delta, redshift, position: (pos_x, pos_y) }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Predicts all three observables for a planetary system at time `t`.
///
/// Contributions from multiple planets are superposed linearly.
pub fn predict_observations(planets: &[Planet], time: f32, inclination: f32) -> Observations {
    let mut obs = Observations {
        brightness: BRIGHTNESS_BASE,
        redshift:   0.0,
        position:   (0.0, 0.0),
    };
    for planet in planets {
        let c = planet_contribution(planet, time, inclination);
        obs.brightness += c.brightness;
        obs.redshift   += c.redshift;
        obs.position.0 += c.position.0;
        obs.position.1 += c.position.1;
    }
    obs
}

/// Returns the sub-pixel `(dx, dy)` offset used to animate the star texture,
/// consistent with the same orbital model used for the graphs.
///
/// Output is in pixel units, suitable for passing directly to `generate_star`.
pub fn star_pixel_offset(planets: &[Planet], time: f32, inclination: f32) -> (f32, f32) {
    let mut dx = 0.0_f32;
    let mut dy = 0.0_f32;
    for planet in planets {
        let angle = orbital_angle(time, planet.period, planet.eccentricity);
        let perp  = (-planet.direction.1, planet.direction.0);
        let amp   = planet.mass * PIXEL_WOBBLE_PER_MASS;
        dx += planet.direction.0 * angle.cos() * amp
            + perp.0              * angle.sin() * amp * (1.0 - inclination);
        dy += planet.direction.1 * angle.cos() * amp
            + perp.1              * angle.sin() * amp * (1.0 - inclination);
    }
    (dx, dy)
}
