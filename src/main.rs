use macroquad::prelude::*;

use macroquad::ui::widgets::{Slider};
use macroquad::ui::{
    hash, root_ui,
    widgets::{self, Group},
    Drag, Ui,
};

mod cpal_test;
mod dasp_test;

#[macroquad::main("Texture")]
async fn main() {
    let texture: Texture2D = load_texture("assets/ferris.png").await.expect("Failed to load texture");

    let sample_rate = cpal_test::get_sample_rate().expect("Failed to init audio");
    let signal = dasp_test::create_signal(sample_rate);
    // _stream MUST be named (not `let _ = ...`) or audio stops right away
    let _stream = cpal_test::init_stream(signal).expect("Failed to start audio");

    loop {
        clear_background(LIGHTGRAY);
        let window_w = screen_width();
        let tw = texture.width();
        let th = texture.height();
        let dest_size = vec2(window_w, th / tw * window_w);
        
        draw_texture_ex(&texture,
            0., 0.,
            WHITE,
            DrawTextureParams {
                dest_size: Some(dest_size),
                ..Default::default()
            },
        );

        // create a fake spectrum
        // display it

        // UI
        let pos = vec2(400., 200.);
        let size = vec2(320., 400.);
        widgets::Window::new(hash!(), pos, size)
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
