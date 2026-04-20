use macroquad::prelude::*;

mod cpal_test;
mod dasp_test;
mod spectrum;
mod spectral_lines;
mod star;
mod scenes;

use scenes::{GameScene, InitialFadeIn};

#[derive(Copy, Clone)]
struct GameState {
    round: u16,
    rng_seed: u64,
    scene: GameScene,
}

impl GameState {
    fn new() -> GameState {
        GameState {
            round: 1,
            rng_seed: 0xABCD_1234,
            scene: GameScene::InitialFadeIn(InitialFadeIn::new()),
        }
    }

    fn update(self) -> GameState {
        GameState { scene: self.scene.update(), ..self }
    }

    fn draw(&self) {
        clear_background(BLACK);
        // TODO: draw starfield here (visible in all scenes)
        self.scene.draw();
    }
}

#[macroquad::main("Survey")]
async fn main() {
    let mut state = GameState::new();

    // TODO: generate starfield texture from rng_seed

    loop {
        state = state.update();
        state.draw();
        next_frame().await
    }
}
