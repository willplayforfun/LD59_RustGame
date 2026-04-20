use macroquad::prelude::*;
use super::GameScene;
use super::star_analysis::StarAnalysis;
use crate::world::World;

const IDENTIFY_DURATION: f32 = 3.0;

#[derive(Copy, Clone)]
pub struct StarIdentify {
    pub progress: f32,
    pub selected_star: usize,
}

impl StarIdentify {
    pub fn new(selected_star: usize) -> Self {
        StarIdentify { progress: 0.0, selected_star }
    }

    pub fn update(self, _world: &World) -> GameScene {
        let p = self.progress + get_frame_time() / IDENTIFY_DURATION;
        if p >= 1.0 {
            GameScene::StarAnalysis(StarAnalysis::new(self.selected_star))
        } else {
            GameScene::StarIdentify(StarIdentify { progress: p, ..self })
        }
    }

    pub fn draw(&self) {
        // TODO: box starts at screen edge and narrows in to the selected star
    }
}
