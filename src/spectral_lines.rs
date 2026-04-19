use std::collections::HashMap;
use crate::spectrum::{GaussianDist, Spectrum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpectralSource {
    Hydrogen,
    Helium,
    Sodium,
    Calcium,
    Oxygen,
    Mercury,
    Neon,
    WaterVapor,
}

fn spectral_lines(source: SpectralSource) -> Vec<GaussianDist> {
    // Wavelengths in nm, amplitudes are relative line strengths (0-1).
    // sdev of ~2nm approximates typical narrow atomic line widths.
    match source {
        SpectralSource::Hydrogen => vec![
            GaussianDist { mean: 656.3, sdev: 2.0, ampl: 1.0 },  // Hα
            GaussianDist { mean: 486.1, sdev: 2.0, ampl: 0.7 },  // Hβ
            GaussianDist { mean: 434.0, sdev: 2.0, ampl: 0.4 },  // Hγ
            GaussianDist { mean: 410.2, sdev: 2.0, ampl: 0.2 },  // Hδ
        ],
        SpectralSource::Helium => vec![
            GaussianDist { mean: 587.6, sdev: 2.0, ampl: 1.0 },  // D3
            GaussianDist { mean: 501.6, sdev: 2.0, ampl: 0.6 },
            GaussianDist { mean: 447.1, sdev: 2.0, ampl: 0.5 },
            GaussianDist { mean: 438.8, sdev: 2.0, ampl: 0.3 },
        ],
        SpectralSource::Sodium => vec![
            GaussianDist { mean: 589.0, sdev: 2.0, ampl: 1.0 },  // D2
            GaussianDist { mean: 589.6, sdev: 2.0, ampl: 0.9 },  // D1
        ],
        SpectralSource::Calcium => vec![
            GaussianDist { mean: 422.7, sdev: 2.0, ampl: 1.0 },
            GaussianDist { mean: 445.5, sdev: 2.0, ampl: 0.5 },
        ],
        SpectralSource::Oxygen => vec![
            GaussianDist { mean: 630.0, sdev: 2.0, ampl: 0.8 },
            GaussianDist { mean: 636.4, sdev: 2.0, ampl: 0.5 },
            GaussianDist { mean: 557.7, sdev: 2.0, ampl: 1.0 },
        ],
        SpectralSource::Mercury => vec![
            GaussianDist { mean: 435.8, sdev: 2.0, ampl: 0.8 },
            GaussianDist { mean: 546.1, sdev: 2.0, ampl: 1.0 },
            GaussianDist { mean: 578.0, sdev: 2.0, ampl: 0.7 },
        ],
        SpectralSource::Neon => vec![
            GaussianDist { mean: 585.2, sdev: 2.0, ampl: 0.6 },
            GaussianDist { mean: 594.5, sdev: 2.0, ampl: 0.7 },
            GaussianDist { mean: 614.3, sdev: 2.0, ampl: 0.9 },
            GaussianDist { mean: 640.2, sdev: 2.0, ampl: 1.0 },
            GaussianDist { mean: 703.2, sdev: 2.0, ampl: 0.5 },
        ],
        SpectralSource::WaterVapor => vec![
            GaussianDist { mean: 650.0, sdev: 15.0, ampl: 0.4 },  // broad H2O band
            GaussianDist { mean: 720.0, sdev: 20.0, ampl: 0.7 },  // broad H2O band
        ],
    }
}

/// Builds a Spectrum from a map of spectral sources to fractional abundances (0.0–1.0).
/// Each source's lines are scaled by its abundance and collected into `subs`.
pub fn build_spectrum(abundances: &HashMap<SpectralSource, f32>) -> Spectrum {
    let mut subs = Vec::new();
    for (&source, &abundance) in abundances {
        for line in spectral_lines(source) {
            subs.push(GaussianDist {
                mean: line.mean,
                sdev: line.sdev,
                ampl: line.ampl * abundance,
            });
        }
    }
    Spectrum { base: 1.0, adds: vec![], subs }
}
