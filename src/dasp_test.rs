use dasp::{Frame, Sample, Signal};
use dasp::signal::{self, Sine};
use hound::{SampleFormat, WavSpec, WavWriter};

pub fn create_signal(sample_rate: f64) -> impl Signal<Frame = f64> + Send {
    // Create a sine wave oscillator at 440 Hz
    let sine = signal::rate(sample_rate as f64).const_hz(440.0).sine();
    
    // Create a tremolo effect (amplitude modulation)
    let tremolo_freq = 5.0; // 5 Hz tremolo
    let tremolo = signal::rate(sample_rate as f64).const_hz(tremolo_freq).sine();
    
    // Apply the tremolo to the sine wave
    let signal = sine.zip_map(tremolo, |sine, tremolo| {
        // Map tremolo from [-1, 1] to [0.5, 1.0] range for amplitude modulation
        let amplitude = 0.75 + 0.25 * tremolo;
        sine * amplitude
    });
    
    return signal;
}