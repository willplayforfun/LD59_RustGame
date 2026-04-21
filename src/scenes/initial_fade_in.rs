use macroquad::prelude::*;
use super::GameScene;
use super::star_identify::StarIdentify;
use crate::world::World;

const FADE_IN_END:   f32 = 1.0;
const TYPE_END:      f32 = 2.0;
const HOLD_END:      f32 = 2.8;
const FADE_OUT_END:  f32 = 3.3;

const FONT_SIZE: f32 = 60.0;

#[derive(Copy, Clone)]
pub struct InitialFadeIn {
    pub round:         u8,
    pub selected_star: usize,
    pub elapsed:       f32,
}

impl InitialFadeIn {
    pub fn new(round: u8, selected_star: usize) -> Self {
        InitialFadeIn { round, selected_star, elapsed: 0.0 }
    }

    pub fn new_returning(round: u8, selected_star: usize) -> Self {
        InitialFadeIn { round, selected_star, elapsed: FADE_IN_END }
    }

    pub fn update(self, _world: &mut World) -> GameScene {
        let elapsed = self.elapsed + get_frame_time();
        if elapsed >= FADE_OUT_END {
            GameScene::StarIdentify(StarIdentify::new(self.selected_star, self.round))
        } else {
            GameScene::InitialFadeIn(InitialFadeIn { elapsed, ..self })
        }
    }

    pub fn draw(&self, _world: &World) {
        let t = self.elapsed;

        // Phase 1: black overlay fades out to reveal starfield.
        if t < FADE_IN_END {
            let alpha = 1.0 - (t / FADE_IN_END);
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(),
                Color::new(0.0, 0.0, 0.0, alpha));
            return;
        }

        let label = format!("Survey {}", self.round);
        let total_chars = label.len();

        let chars_shown = if t < TYPE_END {
            let frac = (t - FADE_IN_END) / (TYPE_END - FADE_IN_END);
            (frac * total_chars as f32).ceil() as usize
        } else {
            total_chars
        };

        let text_alpha = if t < HOLD_END {
            1.0f32
        } else {
            1.0 - (t - HOLD_END) / (FADE_OUT_END - HOLD_END)
        };

        let visible = &label[..chars_shown.min(total_chars)];
        let dims = measure_text(visible, None, FONT_SIZE as u16, 1.0);
        let x = (screen_width()  - dims.width)  / 2.0;
        let y = (screen_height() + dims.height) / 2.0;

        draw_text(visible, x, y, FONT_SIZE, Color::new(1.0, 1.0, 1.0, text_alpha));
    }
}
