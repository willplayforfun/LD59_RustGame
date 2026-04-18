use macroquad::prelude::*;

#[macroquad::main("Texture")]
async fn main() {
    let texture: Texture2D = load_texture("assets/ferris.png").await.unwrap();

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
        next_frame().await
    }
}
