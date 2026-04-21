// Star and starfield pixel rendering.
//
// Pipeline:
//   1. gaussian_psf()             — build the optical blur kernel
//   2. color_temperature_to_rgb() — blackbody colour from Kelvin
//   3. generate_star()            — stamp PSF, bloom, colorize → RGBA pixels
//   4. generate_starfield()       — scatter many stars into a texture buffer

use crate::rng::Rng;
use crate::star::{generate_star_data, star_seed};

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

    let half = (size / 2) as i32;
    let two_sigma_sq = 2.0 * sigma * sigma;

    let mut weights: Vec<f32> = (0..size * size)
        .map(|i| {
            let x = (i % size) as i32 - half;
            let y = (i / size) as i32 - half;
            let dist_sq = (x * x + y * y) as f32;
            f32::exp(-dist_sq / two_sigma_sq)
        })
        .collect();

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
/// Uses the Tanner Helland piecewise approximation. Valid range is roughly
/// 1 000 K–40 000 K; values outside are clamped.
///
/// The output is **linear light**, not gamma-encoded.
pub fn color_temperature_to_rgb(kelvin: f32) -> (f32, f32, f32) {
    let t = (kelvin / 100.0).clamp(10.0, 400.0);

    let red = if t <= 66.0 {
        1.0
    } else {
        (329.698_727_4 * (t - 60.0).powf(-0.133_204_76) / 255.0).clamp(0.0, 1.0)
    };

    let green = if t <= 66.0 {
        ((99.470_802_6 * t.ln() - 161.119_568_2) / 255.0).clamp(0.0, 1.0)
    } else {
        (288.122_169_5 * (t - 60.0).powf(-0.075_514_85) / 255.0).clamp(0.0, 1.0)
    };

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
/// `size`       — side length in pixels, must be odd and ≤ 32.
/// `kelvin`     — blackbody colour temperature (1 000–40 000 K).
/// `brightness` — peak intensity as a multiple of sensor saturation.
/// `psf`        — optical blur kernel (see `gaussian_psf`).
/// `sub_pixel`  — fractional pixel offset of the star centre, range (-0.5, 0.5).
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

    let mut lum = stamp_psf(size, brightness, psf, sub_pixel);
    bloom(&mut lum, size, 4);

    let (sr, sg, sb) = color_temperature_to_rgb(kelvin);

    let mut rgba = Vec::with_capacity(size * size * 4);
    for &l in &lum {
        let (r, g, b) = colorize(l, sr, sg, sb);
        rgba.push(to_srgb_u8(r));
        rgba.push(to_srgb_u8(g));
        rgba.push(to_srgb_u8(b));
        rgba.push(to_srgb_u8(l.min(1.0)));
    }
    rgba
}

// ─── Starfield Texture ───────────────────────────────────────────────────────

/// Generates a static starfield as a `width × height` RGBA texture.
///
/// Returns a `Vec<u8>` of length `width * height * 4`, ready for
/// `Texture2D::from_rgba8`.
///
/// Stars are additively composited in **linear light** before gamma encoding,
/// so overlapping halos add correctly without colour banding.
///
/// `density` — average stars per 10 000 px². 2.0–4.0 produces a natural field.
/// `seed`    — RNG seed; the same seed always produces the same field.
pub fn generate_starfield(
    width: usize,
    height: usize,
    density: f32,
    psf: &PsfKernel,
    seed: u64,
) -> Vec<u8> {
    // IMPORTANT: difficulty is fixed at 1 here — the starfield is purely visual
    // and planet count does not affect star position, colour, or brightness.
    const VISUAL_DIFFICULTY: u8 = 1;

    let star_count = ((width * height) as f32 * density / 10_000.0).round() as usize;

    let mut r_buf = vec![0.0f32; width * height];
    let mut g_buf = vec![0.0f32; width * height];
    let mut b_buf = vec![0.0f32; width * height];

    for i in 0..star_count {
        let data = generate_star_data(seed, i, VISUAL_DIFFICULTY);
        let cx = data.position.0 * width  as f32;
        let cy = data.position.1 * height as f32;

        let pixel_x = cx.round() as i32;
        let pixel_y = cy.round() as i32;
        let sub     = (cx - pixel_x as f32, cy - pixel_y as f32);

        // Use a separate per-star RNG for visual-only properties (star size).
        let mut rng   = Rng::new(star_seed(seed, i).wrapping_add(0x5EED_5175));
        let star_size = random_star_size(&mut rng);
        let half      = (star_size / 2) as i32;

        let mut lum = stamp_psf(star_size, data.brightness, psf, sub);
        bloom(&mut lum, star_size, 4);

        let (sr, sg, sb) = color_temperature_to_rgb(data.temperature);

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

    let mut rgba = Vec::with_capacity(width * height * 4);
    for i in 0..width * height {
        rgba.push(to_srgb_u8(r_buf[i]));
        rgba.push(to_srgb_u8(g_buf[i]));
        rgba.push(to_srgb_u8(b_buf[i]));
        rgba.push(255);
    }
    rgba
}

// ─── Private helpers ─────────────────────────────────────────────────────────

fn stamp_psf(
    size: usize,
    brightness: f32,
    psf: &PsfKernel,
    sub_pixel: (f32, f32),
) -> Vec<f32> {
    let mut buf    = vec![0.0f32; size * size];
    let center     = (size / 2) as f32;
    let scale      = brightness / psf_sample_bilinear(psf, 0.0, 0.0);

    for py in 0..size {
        for px in 0..size {
            let dx = px as f32 - (center + sub_pixel.0);
            let dy = py as f32 - (center + sub_pixel.1);
            buf[py * size + px] = scale * psf_sample_bilinear(psf, dx, dy);
        }
    }
    buf
}

fn psf_sample_bilinear(psf: &PsfKernel, dx: f32, dy: f32) -> f32 {
    let half = (psf.size / 2) as f32;
    let kx   = dx + half;
    let ky   = dy + half;

    let x0 = kx.floor() as i32;
    let y0 = ky.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let tx  = kx - kx.floor();
    let ty  = ky - ky.floor();

    let w = |x: i32, y: i32| -> f32 {
        if x < 0 || x >= psf.size as i32 || y < 0 || y >= psf.size as i32 {
            0.0
        } else {
            psf.weights[y as usize * psf.size + x as usize]
        }
    };

    let top    = lerp(w(x0, y0), w(x1, y0), tx);
    let bottom = lerp(w(x0, y1), w(x1, y1), tx);
    lerp(top, bottom, ty)
}

/// Redistributes energy from saturated pixels to cardinal neighbours,
/// simulating CCD charge overflow (blooming).
fn bloom(lum: &mut [f32], size: usize, iterations: u32) {
    for _ in 0..iterations {
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
                if x > 0        { lum[y * size + x - 1]   += share; }
                if x < size - 1 { lum[y * size + x + 1]   += share; }
                if y > 0        { lum[(y - 1) * size + x] += share; }
                if y < size - 1 { lum[(y + 1) * size + x] += share; }
            }
        }
    }
}

/// Picks a star texture size (odd pixels) from a weighted distribution.
///
/// | size | weight | appearance                    |
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

#[inline]
fn colorize(lum: f32, sr: f32, sg: f32, sb: f32) -> (f32, f32, f32) {
    let display     = lum.min(1.0);
    let white_blend = display * display;
    (
        display * lerp(sr, 1.0, white_blend),
        display * lerp(sg, 1.0, white_blend),
        display * lerp(sb, 1.0, white_blend),
    )
}

#[inline]
fn to_srgb_u8(linear: f32) -> u8 {
    (linear.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
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
        let k      = gaussian_psf(9, 2.0);
        let center = k.weights[4 * 9 + 4];
        for &w in &k.weights {
            assert!(w <= center + 1e-6);
        }
    }

    #[test]
    #[should_panic]
    fn even_size_panics() {
        gaussian_psf(8, 2.0);
    }

    #[test]
    fn star_output_has_correct_length() {
        let psf  = gaussian_psf(9, 2.0);
        let rgba = generate_star(15, 6500.0, 3.0, &psf, (0.0, 0.0));
        assert_eq!(rgba.len(), 15 * 15 * 4);
    }

    #[test]
    fn star_centre_is_near_white() {
        let psf    = gaussian_psf(9, 2.0);
        let rgba   = generate_star(15, 6500.0, 10.0, &psf, (0.0, 0.0));
        let centre = 7 * 15 + 7;
        let r = rgba[centre * 4];
        let g = rgba[centre * 4 + 1];
        let b = rgba[centre * 4 + 2];
        assert!(r > 240 && g > 240 && b > 240, "got ({r},{g},{b})");
    }

    #[test]
    fn star_edges_are_darker_than_centre() {
        let psf    = gaussian_psf(9, 2.0);
        let rgba   = generate_star(15, 6500.0, 3.0, &psf, (0.0, 0.0));
        let centre = (rgba[(7 * 15 + 7) * 4] as u32
                    + rgba[(7 * 15 + 7) * 4 + 1] as u32
                    + rgba[(7 * 15 + 7) * 4 + 2] as u32) / 3;
        let corner = (rgba[0] as u32 + rgba[1] as u32 + rgba[2] as u32) / 3;
        assert!(centre > corner);
    }

    #[test]
    fn cool_star_halo_is_reddish() {
        let psf  = gaussian_psf(9, 2.0);
        let rgba = generate_star(15, 3000.0, 0.8, &psf, (0.0, 0.0));
        let idx  = (7 * 15 + 7) * 4;
        assert!(rgba[idx] > rgba[idx + 2], "3000K should be redder than blue");
    }

    #[test]
    fn starfield_output_has_correct_length() {
        let psf  = gaussian_psf(9, 2.0);
        let rgba = generate_starfield(160, 120, 2.0, &psf, 42);
        assert_eq!(rgba.len(), 160 * 120 * 4);
    }

    #[test]
    fn starfield_is_not_all_black() {
        let psf     = gaussian_psf(9, 2.0);
        let rgba    = generate_starfield(160, 120, 5.0, &psf, 42);
        let any_lit = rgba.chunks(4).any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0);
        assert!(any_lit);
    }

    #[test]
    fn starfield_is_deterministic() {
        let psf = gaussian_psf(9, 2.0);
        assert_eq!(
            generate_starfield(80, 60, 2.0, &psf, 99),
            generate_starfield(80, 60, 2.0, &psf, 99),
        );
    }

    #[test]
    fn starfield_differs_with_different_seed() {
        let psf = gaussian_psf(9, 2.0);
        assert_ne!(
            generate_starfield(80, 60, 2.0, &psf, 1),
            generate_starfield(80, 60, 2.0, &psf, 2),
        );
    }

    #[test]
    fn sub_pixel_offset_changes_edge_pixels() {
        let psf     = gaussian_psf(9, 2.0);
        let centred = generate_star(15, 6500.0, 3.0, &psf, (0.0, 0.0));
        let shifted = generate_star(15, 6500.0, 3.0, &psf, (0.4, 0.0));
        let right   = (7 * 15 + 11) * 4;
        let left    = (7 * 15 +  3) * 4;
        assert!(shifted[right] > centred[right], "right tail should brighten");
        assert!(shifted[left]  < centred[left],  "left tail should dim");
    }

    #[test]
    fn daylight_is_near_white() {
        let (r, g, b) = color_temperature_to_rgb(6500.0);
        assert!((r - g).abs() < 0.1);
        assert!((g - b).abs() < 0.1);
    }

    #[test]
    fn warm_star_is_redder_than_blue() {
        let (r, _g, b) = color_temperature_to_rgb(3000.0);
        assert!(r > b);
    }

    #[test]
    fn hot_star_is_bluer_than_red() {
        let (r, _g, b) = color_temperature_to_rgb(15_000.0);
        assert!(b > r);
    }

    #[test]
    fn all_channels_in_range() {
        for kelvin in [1000.0, 3000.0, 6500.0, 10_000.0, 30_000.0] {
            let (r, g, b) = color_temperature_to_rgb(kelvin);
            assert!((0.0..=1.0).contains(&r));
            assert!((0.0..=1.0).contains(&g));
            assert!((0.0..=1.0).contains(&b));
        }
    }
}
