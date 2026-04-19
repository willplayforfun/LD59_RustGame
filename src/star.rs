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
        let display = l.min(1.0); // clamp: can't show more than full brightness

        // Blend toward white as the pixel saturates.  At low luminance the star
        // shows its blackbody colour; at high luminance all channels clip to 1.
        // display² gives a smooth but rapid transition to white near the core.
        let white_blend = display * display;
        let r = display * lerp(sr, 1.0, white_blend);
        let g = display * lerp(sg, 1.0, white_blend);
        let b = display * lerp(sb, 1.0, white_blend);

        // Convert linear light → sRGB (gamma 2.2) and write bytes.
        rgba.push(to_srgb_u8(r));
        rgba.push(to_srgb_u8(g));
        rgba.push(to_srgb_u8(b));
        rgba.push(to_srgb_u8(display)); // alpha tracks luminance; black = transparent
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
