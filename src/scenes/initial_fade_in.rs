use macroquad::prelude::*;
use super::GameScene;
use super::star_identify::StarIdentify;
use crate::world::World;

const FADE_DURATION: f32 = 2.0;

#[derive(Copy, Clone)]
pub struct InitialFadeIn {
    pub progress: f32,
}

impl InitialFadeIn {
    pub fn new() -> Self {
        InitialFadeIn { progress: 0.0 }
    }

    pub fn update(self, _world: &World) -> GameScene {
        let p = self.progress + get_frame_time() / FADE_DURATION;
        if p >= 1.0 {
            GameScene::StarIdentify(StarIdentify::new(0))
        } else {
            GameScene::InitialFadeIn(InitialFadeIn { progress: p })
        }
    }

    pub fn draw(&self) {
        let alpha = 1.0 - self.progress;
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(),
            Color::new(0.0, 0.0, 0.0, alpha));
    }
}
