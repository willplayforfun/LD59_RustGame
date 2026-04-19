use macroquad::prelude::*;

use macroquad::ui::{
    hash, root_ui,
    widgets::{self, Group},
};

mod cpal_test;
mod dasp_test;
mod spectrum;
mod spectral_lines;
mod star;

use std::collections::HashMap;
use spectrum::SpectrumRenderer;
use spectral_lines::{SpectralSource, build_spectrum};

#[macroquad::main("Texture")]
async fn main() {
    let texture: Texture2D = load_texture("assets/ferris.png").await.expect("Failed to load texture");

    let sample_rate = cpal_test::get_sample_rate().expect("Failed to init audio");
    let signal = dasp_test::create_signal(sample_rate);
    let _stream = cpal_test::init_stream(signal).expect("Failed to start audio");

    let mut renderer = SpectrumRenderer::new(400.0, 750.0, 1.2).await;

    let abundances = HashMap::from([
        (SpectralSource::Hydrogen, 0.9),
        (SpectralSource::Sodium,   0.3),
    ]);
    let test_spectrum = build_spectrum(&abundances);
    renderer.update(&test_spectrum);

    loop {
        clear_background(LIGHTGRAY);
        let window_w = screen_width();
        let tw = texture.width();
        let th = texture.height();
        let dest_size = vec2(window_w, th / tw * window_w);

        draw_texture_ex(&texture, 0., 0., WHITE, DrawTextureParams {
            dest_size: Some(dest_size),
            ..Default::default()
        });

        renderer.draw(
            vec2(screen_width() * 0.25, screen_height() * 0.5),
            vec2(screen_width() * 0.4, screen_height() * 0.4),
        );

        // UI
        widgets::Window::new(hash!(), vec2(400., 200.), vec2(320., 400.))
            .label("Shop")
            .titlebar(true)
            .ui(&mut *root_ui(), |ui| {
                for i in 0..30 {
                    Group::new(hash!("shop", i), Vec2::new(300., 80.))
                        .ui(ui, |ui| {
                            ui.label(Vec2::new(10., 10.), &format!("Item N {i}"));
                            ui.label(Vec2::new(260., 40.), "10/10");
                        });
                }
            });

        next_frame().await
    }
}
