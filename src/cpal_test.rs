// Using CPAL for cross-platform audio I/O
// Cargo.toml:
// [dependencies]
// cpal = "0.15"
// anyhow = "1.0"

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat};
use dasp::{Frame, Signal};
use cpal::{SizedSample, FromSample};

pub fn get_sample_rate() -> Result<f64, anyhow::Error>
{
    // Get the default host
    let host = cpal::default_host();
    
    // Get the default output device
    let device = host.default_output_device()
        .expect("No output device available");

    // Get the default output config
    let config = device.default_output_config()?;
    
    let sample_rate = config.sample_rate().0 as f64;
    Ok(sample_rate)
}

pub fn init_stream(
    mut signal: impl Signal<Frame = f64> + Send + 'static
) 
-> Result<cpal::Stream, anyhow::Error> 
{
    // Get the default host
    let host = cpal::default_host();
    
    // Get the default output device
    let device = host.default_output_device()
        .expect("No output device available");
    
    println!("Output device: {}", device.name()?);
    
    // Get the default output config
    let config = device.default_output_config()?;
    println!("Default output config: {:?}", config);
    
    let channels = config.channels() as usize;

    // Build an output stream
    let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
    
    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _| fill_buffer(data, channels, &mut signal),
            err_fn,
            None,
        )?,
        SampleFormat::I16 => device.build_output_stream(
            &config.into(),
            move |data: &mut [i16], _| fill_buffer(data, channels, &mut signal),
            err_fn,
            None,
        )?,
        SampleFormat::U16 => device.build_output_stream(
            &config.into(),
            move |data: &mut [u16], _| fill_buffer(data, channels, &mut signal),
            err_fn,
            None,
        )?,
        _ => return Err(anyhow::Error::msg("Unsupported sample format")),
    };

    stream.play()?;
    Ok(stream)
}

fn fill_buffer<T: SizedSample + FromSample<f32>>(
    output: &mut [T],
    channels: usize,
    signal: &mut impl Signal<Frame = f64>,
) {
    for frame in output.chunks_mut(channels) {
        let value = T::from_sample(signal.next() as f32);
        for sample in frame.iter_mut() { *sample = value; }
    }
}