use macroquad::prelude::*;

mod cpal_test;
mod dasp_test;
mod spectrum;
mod spectral_lines;
mod rng;
mod star;
mod star_rendering;
mod world;
mod scenes;

use scenes::{GameScene, InitialFadeIn};
use star_rendering::{gaussian_psf, generate_starfield};
use world::World;

struct GameState {
    round: u16,
    world: World,
    scene: GameScene,
}

impl GameState {
    async fn init() -> Self {
        let seed = 0xABCD_1234u64;
        let psf  = gaussian_psf(9, 2.0);
        let w    = screen_width()  as usize;
        let h    = screen_height() as usize;
        let pixels   = generate_starfield(w, h, 2.0, &psf, seed);
        let starfield = Texture2D::from_rgba8(w as u16, h as u16, &pixels);

        GameState {
            round: 1,
            world: World { seed, starfield },
            scene: GameScene::InitialFadeIn(InitialFadeIn::new()),
        }
    }

    fn update(self) -> Self {
        let scene = self.scene.update(&self.world);
        GameState { scene, ..self }
    }

    fn draw(&self) {
        clear_background(BLACK);
        draw_texture_ex(
            &self.world.starfield,
            0.0, 0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(screen_width(), screen_height())),
                ..Default::default()
            },
        );
        self.scene.draw();
    }
}

#[macroquad::main("Survey")]
async fn main() {
    let mut state = GameState::init().await;
    loop {
        state = state.update();
        state.draw();
        next_frame().await
    }
}
