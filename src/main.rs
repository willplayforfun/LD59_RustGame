use macroquad::prelude::*;
use macroquad::ui::{root_ui, Skin};

mod cpal_test;
mod dasp_test;
mod spectrum;
mod spectral_lines;
mod rng;
mod star;
mod star_rendering;
mod simulation;
mod world;
mod scenes;

use scenes::{GameScene, InitialFadeIn};
use star_rendering::{gaussian_psf, generate_starfield, starfield_star_count};
use world::World;

struct GameState {
    round:         u8,
    selected_star: usize,
    world:         World,
    scene:         GameScene,
}

impl GameState {
    async fn init() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xABCD_1234u64);
        let psf  = gaussian_psf(9, 2.0);
        let w    = screen_width()  as usize;
        let h    = screen_height() as usize;
        let pixels   = generate_starfield(w, h, 2.0, &psf, seed);
        let starfield = Texture2D::from_rgba8(w as u16, h as u16, &pixels);

        let ui_skin = {
            let default = root_ui().default_skin();

            let label_style = root_ui()
                .style_builder()
                .margin(RectOffset::new(0.0, 0.0, 0.0, 0.0))
                .build();

            // Rebuild window styles with color_inactive matching the active colour,
            // disabling the dimming effect when the window loses focus.
            // Active colours sourced from macroquad's default skin (style.rs).
            let window_style = root_ui()
                .style_builder()
                .background(Image {
                    width: 3, height: 3,
                    bytes: vec![
                        68,68,68,255,  68,68,68,255,  68,68,68,255,
                        68,68,68,255,  238,238,238,255, 68,68,68,255,
                        68,68,68,255,  68,68,68,255,  68,68,68,255,
                    ],
                })
                .background_margin(RectOffset::new(1., 1., 1., 1.))
                .color_inactive(Color::from_rgba(238, 238, 238, 255))
                .text_color(Color::from_rgba(0, 0, 0, 255))
                .build();

            let window_titlebar_style = root_ui()
                .style_builder()
                .color(Color::from_rgba(68, 68, 68, 255))
                .color_inactive(Color::from_rgba(68, 68, 68, 255))
                .text_color(Color::from_rgba(0, 0, 0, 255))
                .font_size(26)
                .build();

            Skin { label_style, window_style, window_titlebar_style, ..default }
        };

        let ui_skin_heading = {
            let default = root_ui().default_skin();
            let label_style = root_ui()
                .style_builder()
                .margin(RectOffset::new(0.0, 0.0, 0.0, 0.0))
                .font_size(20)
                .text_color(Color::from_rgba(0, 0, 0, 255))
                .build();
            Skin { label_style, ..default }
        };

        // Placeholder star texture — overwritten immediately by StarAnalysis::new.
        const STAR_TEX_PX: u16 = 15;
        let star_tex = Texture2D::from_rgba8(
            STAR_TEX_PX, STAR_TEX_PX,
            &vec![0u8; (STAR_TEX_PX * STAR_TEX_PX * 4) as usize],
        );
        star_tex.set_filter(FilterMode::Nearest);

        let star_count = starfield_star_count(w, h, 2.0);
        let world = World { seed, star_count, starfield, ui_skin, ui_skin_heading, psf, star_tex };

        let round = 1;
        let selected_star = Self::pick_star(seed, round, world.star_count);
        let scene = GameScene::InitialFadeIn(InitialFadeIn::new(round, selected_star));

        GameState { round, selected_star, world, scene }
    }

    /// Deterministically picks a star index in `0..star_count` for the given round.
    fn pick_star(seed: u64, round: u8, star_count: usize) -> usize {
        let h = seed
            .wrapping_add((round as u64).wrapping_mul(0x9e3779b97f4a7c15));
        (h >> 32) as usize % star_count
    }

    fn update(mut self) -> Self {
        let scene = self.scene.update(&mut self.world);
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
        self.scene.draw(&self.world);
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
