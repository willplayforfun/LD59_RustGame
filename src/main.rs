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
use star::{gaussian_psf, generate_star, planet_wobble_offset};

#[macroquad::main("Texture")]
async fn main() {
    let texture: Texture2D = load_texture("assets/ferris.png").await.expect("Failed to load texture");

    let sample_rate = cpal_test::get_sample_rate().expect("Failed to init audio");
    let signal = dasp_test::create_signal(sample_rate);
    let _stream = cpal_test::init_stream(signal).expect("Failed to start audio");

    // Test star: 15×15 pixels, 6500K (sun-like), moderately overexposed.
    // Sub-pixel offset is updated each frame to simulate a planet wobble.
    let psf        = gaussian_psf(9, 2.0);
    let num_pixels : u16 = 15;
    let star_tex   = Texture2D::from_rgba8(num_pixels, num_pixels,
                         &generate_star(num_pixels as usize, 6500.0, 3.0, &psf, (0.0, 0.0)));
    star_tex.set_filter(FilterMode::Nearest); // keep it chunky when scaled up

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

        // Recompute sub-pixel wobble each frame and upload new star pixels.
        // A 15×15 RGBA texture is tiny so regenerating it every frame is cheap.
        let wobble = planet_wobble_offset(
            get_time() as f32,
            2.0,   // orbital period: 8 seconds per pass
            0.05,  // amplitude: almost half a pixel at closest approach
            0.1,  // sharpness: tight flyby — planet spends ~8% of the period close
            (1.0, 0.0), // direction: mostly horizontal with a slight vertical tilt
        );
        let star_pixels = generate_star(num_pixels as usize, 6500.0, 3.0, &psf, wobble);
        star_tex.update(&Image {
            bytes:  star_pixels,
            width:  num_pixels,
            height: num_pixels,
        });

        // Draw the test star scaled up 16×, centred on the upper portion of the screen.
        let scale     = 16.0;
        let screen_size = vec2(num_pixels as f32 * scale, num_pixels as f32 * scale);
        let star_x    = screen_width()  * 0.5 - screen_size.x * 0.5;
        let star_y    = screen_height() * 0.2 - screen_size.y * 0.5;
        draw_rectangle(star_x, star_y, screen_size.x, screen_size.y, BLACK);
        draw_texture_ex(
            &star_tex,
            star_x,
            star_y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(screen_size),
                ..Default::default()
            },
        );

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
