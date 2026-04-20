use macroquad::prelude::*;
use super::GameScene;

const INTRO_DURATION: f32 = 1.0;

#[derive(Copy, Clone)]
pub struct StarAnalysis {
    pub intro_progress: f32,
    pub selected_star: usize,
}

impl StarAnalysis {
    pub fn new(selected_star: usize) -> Self {
        StarAnalysis { intro_progress: 0.0, selected_star }
    }

    pub fn update(self) -> GameScene {
        let p = (self.intro_progress + get_frame_time() / INTRO_DURATION).min(1.0);
        GameScene::StarAnalysis(StarAnalysis { intro_progress: p, ..self })
    }

    pub fn draw(&self) {
        // TODO: draw star image, brightness/redshift/position graphs, planet UI
        // scale/fade elements in using intro_progress during the opening animation
    }
}
