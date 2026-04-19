/// Procedural star texture generation.
///
/// Pipeline:
///   1. gaussian_psf()             — build the optical blur kernel
///   2. color_temperature_to_rgb() — blackbody color from Kelvin
///   3. generate_star()            — stamp PSF, bloom, colorize → RGBA pixels
///   4. planet_wobble_offset()     — sub-pixel wobble driven by a Lorentzian orbit pulse

// ─── PSF Kernel ──────────────────────────────────────────────────────────────

/// A square convolution kernel representing the optical Point Spread Function.
///
/// `weights` is stored row-major: the weight at column `x`, row `y` is
/// `weights[y * size + x]`. All weights sum to 1.0 so the convolution is
/// energy-conserving.
pub struct PsfKernel {
    /// Side length of the square kernel. Always odd so there is a center pixel.
    pub size: usize,
    /// Normalized weights, length == size * size.
    pub weights: Vec<f32>,
}

/// Builds a Gaussian PSF kernel.
///
/// `size` must be odd (7, 9, 11 …). Panics otherwise.
/// `sigma` is the standard deviation in pixels — controls how far the light
/// spreads. A value of 1.5–2.5 gives a natural-looking soft star core.
pub fn gaussian_psf(size: usize, sigma: f32) -> PsfKernel {
    assert!(size % 2 == 1, "PSF kernel size must be odd");

    let half = (size / 2) as i32; // distance from center to edge in pixels
    let two_sigma_sq = 2.0 * sigma * sigma;

    // Compute raw (un-normalized) Gaussian weights.
    let mut weights: Vec<f32> = (0..size * size)
        .map(|i| {
            // Convert flat index → (x, y) offsets from center.
            let x = (i % size) as i32 - half;
            let y = (i / size) as i32 - half;
            let dist_sq = (x * x + y * y) as f32;
            f32::exp(-dist_sq / two_sigma_sq)
        })
        .collect();

    // Normalize so the kernel sums to exactly 1.0.
    // This ensures convolution moves light around without adding or removing it.
    let sum: f32 = weights.iter().sum();
    for w in &mut weights {
        *w /= sum;
    }

    PsfKernel { size, weights }
}

// ─── Color Temperature ───────────────────────────────────────────────────────

/// Converts a blackbody colour temperature (in Kelvin) to **linear** RGB,
/// each channel in `0.0–1.0`.
///
/// Uses the Tanner Helland piecewise approximation, which fits separate curves
/// below and above 6 600 K (the point where red and blue both saturate).
/// Valid range is roughly 1 000 K–40 000 K; values outside are clamped.
///
/// The output is **linear light**, not gamma-encoded. When you eventually write
/// these values to an 8-bit texture you should apply gamma (e.g. raise each
/// channel to 1/2.2) so they look correct on screen.
pub fn color_temperature_to_rgb(kelvin: f32) -> (f32, f32, f32) {
    // The algorithm works in units of "hundreds of Kelvin" and was fitted for
    // t in [10, 400], i.e. 1 000 K – 40 000 K.
    let t = (kelvin / 100.0).clamp(10.0, 400.0);

    // ── Red ──────────────────────────────────────────────────────────────────
    // Below 6 600 K the sensor is fully saturated in red (value = 255).
    // Above it falls off as a power law.
    let red = if t <= 66.0 {
        1.0
    } else {
        (329.698_727_4 * (t - 60.0).powf(-0.133_204_76) / 255.0).clamp(0.0, 1.0)
    };

    // ── Green ─────────────────────────────────────────────────────────────────
    // Two separate fitted curves either side of 6 600 K.
    let green = if t <= 66.0 {
        ((99.470_802_6 * t.ln() - 161.119_568_2) / 255.0).clamp(0.0, 1.0)
    } else {
        (288.122_169_5 * (t - 60.0).powf(-0.075_514_85) / 255.0).clamp(0.0, 1.0)
    };

    // ── Blue ──────────────────────────────────────────────────────────────────
    // Above 6 600 K the sensor is fully saturated in blue.
    // Below 1 900 K there is no visible blue at all.
    // In between it ramps up logarithmically.
    let blue = if t >= 66.0 {
        1.0
    } else if t <= 19.0 {
        0.0
    } else {
        ((138.517_731_2 * (t - 10.0).ln() - 305.044_792_7) / 255.0).clamp(0.0, 1.0)
    };

    (red, green, blue)
}

// ─── Star Texture ────────────────────────────────────────────────────────────

/// Generates a single star as a square RGBA texture.
///
/// Returns a `Vec<u8>` of length `size * size * 4` (row-major, RGBA order),
/// ready to upload via `Texture2D::from_rgba8`.
///
/// `size`    — side length in pixels, must be odd and ≤ 32.
/// `kelvin`  — blackbody colour temperature (1 000–40 000 K).
/// `brightness` — peak intensity as a multiple of sensor saturation.
///               1.0 = centre pixel just at saturation; 5.0 = strongly overexposed.
///               The PSF stamp is scaled so this value lands exactly at the centre
///               regardless of the kernel shape or sigma.
/// `psf`        — optical blur kernel (see `gaussian_psf`).
/// `sub_pixel`  — fractional pixel offset of the star centre from the texture
///               centre, in the range (-0.5, 0.5) on each axis.  Non-zero values
///               shift the PSF by less than one pixel, which brightens/dims the
///               unsaturated edge pixels as more or less of the PSF tail falls on
///               them — simulating a star drifting by a sub-pixel distance.
///               Pass `(0.0, 0.0)` for a perfectly centred star.
///
/// Alpha is proportional to luminance so that the surrounding black is
/// transparent — useful when drawing a single star as a sprite over a backdrop.
pub fn generate_star(
    size: usize,
    kelvin: f32,
    brightness: f32,
    psf: &PsfKernel,
    sub_pixel: (f32, f32),
) -> Vec<u8> {
    assert!(size % 2 == 1, "star texture size must be odd");
    assert!(size <= 32,    "star texture size must be ≤ 32");

    // ── 1. Stamp PSF at (possibly fractional) centre ──────────────────────────
    // Because the source is a single point, convolution reduces to: evaluate
    // the kernel at each output pixel's displacement from the star centre.
    // A non-zero sub_pixel offset shifts the centre between integer pixels,
    // which is resolved by bilinear interpolation of the kernel weights.
    let mut lum = stamp_psf(size, brightness, psf, sub_pixel);

    // ── 2. Bloom ──────────────────────────────────────────────────────────────
    // Pixels above 1.0 are "saturated". Their excess energy bleeds into
    // cardinal neighbours, simulating CCD charge overflow.
    bloom(&mut lum, size, 4);

    // ── 3. Colorize, apply gamma, pack ───────────────────────────────────────
    let (sr, sg, sb) = color_temperature_to_rgb(kelvin);

    let mut rgba = Vec::with_capacity(size * size * 4);
    for &l in &lum {
        let (r, g, b) = colorize(l, sr, sg, sb);
        rgba.push(to_srgb_u8(r));
        rgba.push(to_srgb_u8(g));
        rgba.push(to_srgb_u8(b));
        rgba.push(to_srgb_u8(l.min(1.0))); // alpha tracks luminance; black = transparent
    }
    rgba
}

// ─── Starfield Texture ───────────────────────────────────────────────────────

/// Generates a static starfield as a `width × height` RGBA texture.
///
/// Returns a `Vec<u8>` of length `width * height * 4`, ready for
/// `Texture2D::from_rgba8`.  Use `FilterMode::Nearest` when rendering so the
/// pixel grid stays sharp when scaled up.
///
/// Stars are additively composited in **linear light** before gamma encoding,
/// so overlapping halos add correctly without colour banding.
///
/// `star_size`  — PSF footprint per star in pixels (odd, ≤ 32).  A value of
/// `density`    — average number of stars per 100 × 100 pixel area of the
///               texture.  A value of 1.0 gives one star per 10 000 px²;
///               2.0 – 4.0 produces a natural-looking field for typical
///               160 × 120 or similar low-res backdrops.
///               Star sizes are chosen randomly per star (see `random_star_size`).
/// `seed`       — RNG seed; the same seed always produces the same field.
pub fn generate_starfield(
    width: usize,
    height: usize,
    density: f32,
    psf: &PsfKernel,
    seed: u64,
) -> Vec<u8> {
    // Derive total star count from texture area and density.
    // density = stars per 10 000 px², so multiply area by density / 10 000.
    let star_count = ((width * height) as f32 * density / 10_000.0).round() as usize;

    let mut rng   = Rng::new(seed);

    // Accumulate star contributions in linear light before gamma encoding.
    let mut r_buf = vec![0.0f32; width * height];
    let mut g_buf = vec![0.0f32; width * height];
    let mut b_buf = vec![0.0f32; width * height];

    for _ in 0..star_count {
        // Random position anywhere in the texture (including near edges — the
        // PSF blit clips out-of-bounds pixels, so halos are naturally cropped).
        let cx = rng.next_f32() * width  as f32;
        let cy = rng.next_f32() * height as f32;

        // Nearest integer pixel centre and the fractional remainder.
        let pixel_x = cx.round() as i32;
        let pixel_y = cy.round() as i32;
        let sub     = (cx - pixel_x as f32, cy - pixel_y as f32);

        // Temperature: uniform across a plausible stellar range.
        let kelvin = rng.range_f32(3_000.0, 20_000.0);

        // Brightness: log-uniform so most stars are dim and a few are bright.
        // exp([-2, 1.5]) spans roughly 0.14 – 4.5.
        let brightness = f32::exp(rng.range_f32(-2.0, 1.5));

        // Size varies per star: mostly tight 3-pixel points, occasionally larger.
        let star_size = random_star_size(&mut rng);
        let half      = (star_size / 2) as i32;

        // Build the linear luminance map and bloom it, as in generate_star.
        let mut lum = stamp_psf(star_size, brightness, psf, sub);
        bloom(&mut lum, star_size, 4);

        let (sr, sg, sb) = color_temperature_to_rgb(kelvin);

        // Splat the star's linear RGB contribution onto the accumulation buffers.
        for sy in 0..star_size {
            for sx in 0..star_size {
                let px = pixel_x + sx as i32 - half;
                let py = pixel_y + sy as i32 - half;
                if px < 0 || px >= width as i32 || py < 0 || py >= height as i32 {
                    continue;
                }
                let (r, g, b) = colorize(lum[sy * star_size + sx], sr, sg, sb);
                let dst = py as usize * width + px as usize;
                r_buf[dst] += r;
                g_buf[dst] += g;
                b_buf[dst] += b;
            }
        }
    }

    // Gamma-encode the accumulated linear values and pack to RGBA.
    // Clamping before gamma handles pixel overlap from bright nearby stars.
    let mut rgba = Vec::with_capacity(width * height * 4);
    for i in 0..width * height {
        rgba.push(to_srgb_u8(r_buf[i]));
        rgba.push(to_srgb_u8(g_buf[i]));
        rgba.push(to_srgb_u8(b_buf[i]));
        rgba.push(255); // fully opaque — starfield has its own black background
    }
    rgba
}

// ─── Private helpers ─────────────────────────────────────────────────────────

/// Evaluates the PSF at each output pixel given a (possibly fractional) star
/// centre, filling a `size×size` luminance buffer.
///
/// `sub_pixel` shifts the star centre by a fraction of a pixel.  Fractional
/// positions are resolved by bilinearly interpolating the kernel weights, so
/// this works for any kernel shape — not just Gaussian.
///
/// Scales so that the on-axis peak equals `brightness` (see `generate_star`).
fn stamp_psf(
    size: usize,
    brightness: f32,
    psf: &PsfKernel,
    sub_pixel: (f32, f32),
) -> Vec<f32> {
    let mut buf    = vec![0.0f32; size * size];
    let center     = (size / 2) as f32;

    // Scale factor: normalise so the peak (at zero displacement) == brightness.
    let centre_weight = psf_sample_bilinear(psf, 0.0, 0.0);
    let scale = brightness / centre_weight;

    for py in 0..size {
        for px in 0..size {
            // Displacement of this output pixel from the (fractional) star centre.
            let dx = px as f32 - (center + sub_pixel.0);
            let dy = py as f32 - (center + sub_pixel.1);
            buf[py * size + px] = scale * psf_sample_bilinear(psf, dx, dy);
        }
    }
    buf
}

/// Samples the PSF kernel at a fractional displacement `(dx, dy)` from its
/// centre using bilinear interpolation.
///
/// Returns 0.0 for displacements outside the kernel footprint.
fn psf_sample_bilinear(psf: &PsfKernel, dx: f32, dy: f32) -> f32 {
    // Convert displacement → kernel array coordinates.
    // The kernel centre is at index (half, half); dx=0 should land there.
    let half = (psf.size / 2) as f32;
    let kx   = dx + half;
    let ky   = dy + half;

    // Integer corners of the bilinear quad.
    let x0 = kx.floor() as i32;
    let y0 = ky.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;

    // Fractional parts — how far we are past the (x0, y0) corner.
    let tx = kx - kx.floor();
    let ty = ky - ky.floor();

    // Helper that returns 0 for out-of-bounds indices.
    let w = |x: i32, y: i32| -> f32 {
        if x < 0 || x >= psf.size as i32 || y < 0 || y >= psf.size as i32 {
            0.0
        } else {
            psf.weights[y as usize * psf.size + x as usize]
        }
    };

    // Standard 2-D bilinear blend: interpolate along x first, then y.
    let top    = lerp(w(x0, y0), w(x1, y0), tx);
    let bottom = lerp(w(x0, y1), w(x1, y1), tx);
    lerp(top, bottom, ty)
}

/// Iteratively redistributes energy from saturated pixels (> 1.0) to their
/// four cardinal neighbours, simulating sensor charge overflow (blooming).
fn bloom(lum: &mut [f32], size: usize, iterations: u32) {
    for _ in 0..iterations {
        // Collect overflow in a separate buffer so this pass doesn't feed
        // into itself — each iteration is a clean redistribution step.
        let mut overflow = vec![0.0f32; lum.len()];
        for i in 0..lum.len() {
            if lum[i] > 1.0 {
                overflow[i] = lum[i] - 1.0;
                lum[i]      = 1.0;
            }
        }
        for y in 0..size {
            for x in 0..size {
                let excess = overflow[y * size + x];
                if excess == 0.0 { continue; }
                let share = excess / 4.0;
                if x > 0        { lum[y * size + x - 1]       += share; }
                if x < size - 1 { lum[y * size + x + 1]       += share; }
                if y > 0        { lum[(y - 1) * size + x]     += share; }
                if y < size - 1 { lum[(y + 1) * size + x]     += share; }
            }
        }
    }
}

/// Picks a star texture size (odd pixels) from a weighted distribution.
///
/// Most stars are tight 3-pixel points; a smaller fraction spread to 5 or 7
/// pixels.  All sizes are odd so there is always a single centre pixel.
///
/// | size | weight | typical appearance            |
/// |------|--------|-------------------------------|
/// |  3   |  70 %  | single bright pixel + faint rim|
/// |  5   |  22 %  | small soft disc               |
/// |  7   |   8 %  | larger halo, rare bright star |
fn random_star_size(rng: &mut Rng) -> usize {
    match rng.next_f32() {
        r if r < 0.70 => 3,
        r if r < 0.92 => 5,
        _             => 7,
    }
}

/// Applies colour temperature and saturation-to-white fade to a luminance value.
/// Returns **linear** (r, g, b) — caller must gamma-encode before writing bytes.
#[inline]
fn colorize(lum: f32, sr: f32, sg: f32, sb: f32) -> (f32, f32, f32) {
    let display     = lum.min(1.0);
    let white_blend = display * display; // smooth, rapid fade to white near core
    (
        display * lerp(sr, 1.0, white_blend),
        display * lerp(sg, 1.0, white_blend),
        display * lerp(sb, 1.0, white_blend),
    )
}

/// Minimal seeded RNG based on xorshift64.
///
/// Not cryptographic, but uniform and fast enough for procedural generation.
/// The same seed always produces the same sequence — useful for reproducible
/// starfields.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Ensure the state is never zero (xorshift is stuck at 0 forever).
        Rng(if seed == 0 { 0xDEAD_BEEF_CAFE_1234 } else { seed })
    }

    fn next_u64(&mut self) -> u64 {
        // Standard xorshift64 constants.
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Returns a uniform float in [0.0, 1.0).
    fn next_f32(&mut self) -> f32 {
        // Use the upper 32 bits for better distribution.
        (self.next_u64() >> 32) as f32 / (u32::MAX as f32 + 1.0)
    }

    fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

/// Linear interpolation between `a` and `b` by factor `t` (0.0 = a, 1.0 = b).
#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ─── Orbital wobble ──────────────────────────────────────────────────────────

/// Returns a sub-pixel `(x, y)` offset that simulates the apparent wobble of a
/// star being tugged by a large planet in an elliptical orbit.
///
/// The displacement follows a **Lorentzian pulse** — `1 / (1 + (phase/τ)²)` —
/// which is exactly the shape of a 1/r² gravitational force integrated over a
/// flyby.  The star sits almost still for most of the orbital period, then
/// receives a brief sharp tug as the planet sweeps through closest approach.
///
/// `time`       — current time in seconds (e.g. from `get_time()`).
/// `period`     — orbital period in seconds.  How long between close passes.
/// `amplitude`  — peak displacement in sub-pixels at closest approach (try 0.3–0.5).
/// `sharpness`  — fraction of the period over which the tug is felt (try 0.05–0.2).
///               Smaller = tighter flyby; larger = slower, more gradual encounter.
/// `direction`  — unit vector `(x, y)` giving the axis of the wobble (the star
///               moves along the projection of its orbit on the sensor plane).
pub fn planet_wobble_offset(
    time: f32,
    period: f32,
    amplitude: f32,
    sharpness: f32,
    direction: (f32, f32),
) -> (f32, f32) {
    // Map time into [-0.5, 0.5] within the current period.
    // 0.0 is the moment of closest approach (periapsis); ±0.5 is the far point.
    let phase = (time / period).fract() - 0.5;

    // Lorentzian pulse: peaks at phase=0, falls off as 1/phase².
    // τ = sharpness controls how much of the period feels "close".
    let tau        = sharpness * 0.5; // half-width in period-fraction units
    let pulse      = 1.0 / (1.0 + (phase / tau).powi(2));

    // Normalise so the peak is exactly `amplitude`, then project onto direction.
    let offset = amplitude * pulse;
    (offset * direction.0, offset * direction.1)
}

/// Converts a linear-light value (0.0–1.0) to a gamma-corrected sRGB byte.
#[inline]
fn to_srgb_u8(linear: f32) -> u8 {
    (linear.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_weights_sum_to_one() {
        let k = gaussian_psf(9, 2.0);
        let sum: f32 = k.weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "weights sum to {sum}");
    }

    #[test]
    fn kernel_center_is_maximum() {
        let k = gaussian_psf(9, 2.0);
        let center = k.weights[4 * 9 + 4]; // middle pixel of a 9×9 grid
        for &w in &k.weights {
            assert!(w <= center + 1e-6, "center should be the peak weight");
        }
    }

    #[test]
    #[should_panic]
    fn even_size_panics() {
        gaussian_psf(8, 2.0);
    }

    // ── generate_star ────────────────────────────────────────────────────────

    #[test]
    fn star_output_has_correct_length() {
        let psf  = gaussian_psf(9, 2.0);
        let rgba = generate_star(15, 6500.0, 3.0, &psf, (0.0, 0.0));
        assert_eq!(rgba.len(), 15 * 15 * 4);
    }

    #[test]
    fn star_centre_is_near_white() {
        let psf  = gaussian_psf(9, 2.0);
        let rgba = generate_star(15, 6500.0, 10.0, &psf, (0.0, 0.0));
        let centre = 7 * 15 + 7;
        let r = rgba[centre * 4];
        let g = rgba[centre * 4 + 1];
        let b = rgba[centre * 4 + 2];
        assert!(r > 240 && g > 240 && b > 240, "centre should be near-white, got ({r},{g},{b})");
    }

    #[test]
    fn star_edges_are_darker_than_centre() {
        let psf    = gaussian_psf(9, 2.0);
        let rgba   = generate_star(15, 6500.0, 3.0, &psf, (0.0, 0.0));
        let centre = (rgba[(7 * 15 + 7) * 4] as u32
                    + rgba[(7 * 15 + 7) * 4 + 1] as u32
                    + rgba[(7 * 15 + 7) * 4 + 2] as u32) / 3;
        let corner = (rgba[0] as u32 + rgba[1] as u32 + rgba[2] as u32) / 3;
        assert!(centre > corner, "centre ({centre}) should be brighter than corner ({corner})");
    }

    #[test]
    fn cool_star_halo_is_reddish() {
        let psf  = gaussian_psf(9, 2.0);
        let rgba = generate_star(15, 3000.0, 0.8, &psf, (0.0, 0.0));
        let idx  = (7 * 15 + 7) * 4;
        let r    = rgba[idx];
        let b    = rgba[idx + 2];
        assert!(r > b, "3000K star centre should be redder than blue, got r={r} b={b}");
    }

    #[test]
    // ── generate_starfield ───────────────────────────────────────────────────

    #[test]
    fn starfield_output_has_correct_length() {
        let psf  = gaussian_psf(9, 2.0);
        let rgba = generate_starfield(160, 120, 2.0, &psf, 42);
        assert_eq!(rgba.len(), 160 * 120 * 4);
    }

    #[test]
    fn starfield_is_not_all_black() {
        // density=5.0 on a 160×120 field → ~96 stars, enough to guarantee lit pixels.
        let psf     = gaussian_psf(9, 2.0);
        let rgba    = generate_starfield(160, 120, 5.0, &psf, 42);
        let any_lit = rgba.chunks(4).any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0);
        assert!(any_lit, "starfield should contain at least one lit pixel");
    }

    #[test]
    fn starfield_is_deterministic() {
        let psf = gaussian_psf(9, 2.0);
        let a   = generate_starfield(80, 60, 2.0, &psf, 99);
        let b   = generate_starfield(80, 60, 2.0, &psf, 99);
        assert_eq!(a, b, "same seed should produce identical output");
    }

    #[test]
    fn starfield_differs_with_different_seed() {
        let psf = gaussian_psf(9, 2.0);
        let a   = generate_starfield(80, 60, 2.0, &psf, 1);
        let b   = generate_starfield(80, 60, 2.0, &psf, 2);
        assert_ne!(a, b, "different seeds should produce different fields");
    }

    #[test]
    fn sub_pixel_offset_changes_edge_pixels() {
        // Shifting the star by 0.4 pixels should brighten pixels in that direction
        // and dim pixels on the opposite side.
        // We test column 11 (row 7) — it sits at the far tail of the 9×9 kernel
        // (displacement +4 from the centre at column 7) so it's unsaturated and
        // sensitive to small shifts. Column 3 is the symmetric counterpart.
        let psf     = gaussian_psf(9, 2.0);
        let centred = generate_star(15, 6500.0, 3.0, &psf, (0.0, 0.0));
        let shifted = generate_star(15, 6500.0, 3.0, &psf, (0.4, 0.0));

        let right_idx = (7 * 15 + 11) * 4; // tail pixel in the direction of the shift
        let left_idx  = (7 * 15 +  3) * 4; // tail pixel opposite the shift

        assert!(
            shifted[right_idx] > centred[right_idx],
            "right tail should brighten when star shifts right"
        );
        assert!(
            shifted[left_idx] < centred[left_idx],
            "left tail should dim when star shifts right"
        );
    }

    // ── color_temperature_to_rgb ─────────────────────────────────────────────

    #[test]
    fn daylight_is_near_white() {
        // ~6 500 K (afternoon sun) should be roughly balanced across channels.
        let (r, g, b) = color_temperature_to_rgb(6500.0);
        assert!((r - g).abs() < 0.1, "R and G should be close at 6500K");
        assert!((g - b).abs() < 0.1, "G and B should be close at 6500K");
    }

    #[test]
    fn warm_star_is_redder_than_blue() {
        // ~3 000 K (cool M-dwarf) should be clearly warm-tinted.
        let (r, _g, b) = color_temperature_to_rgb(3000.0);
        assert!(r > b, "3000K star should have more red than blue");
    }

    #[test]
    fn hot_star_is_bluer_than_red() {
        // ~15 000 K (hot B-type star) should be cool blue-white.
        let (r, _g, b) = color_temperature_to_rgb(15_000.0);
        assert!(b > r, "15000K star should have more blue than red");
    }

    #[test]
    fn all_channels_in_range() {
        for kelvin in [1000.0, 3000.0, 6500.0, 10_000.0, 30_000.0] {
            let (r, g, b) = color_temperature_to_rgb(kelvin);
            assert!((0.0..=1.0).contains(&r), "R out of range at {kelvin}K");
            assert!((0.0..=1.0).contains(&g), "G out of range at {kelvin}K");
            assert!((0.0..=1.0).contains(&b), "B out of range at {kelvin}K");
        }
    }
}
