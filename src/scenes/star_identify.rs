use macroquad::prelude::*;
use super::GameScene;
use super::star_analysis::StarAnalysis;
use crate::world::World;
use crate::star::generate_star_data;

/// How long the box takes to narrow from full-screen to the target star.
const NARROW_END: f32 = 1.8;
/// Total duration including the blink confirmation at the end.
const TOTAL_END:  f32 = 3.0;

const BOX_PAD:    f32 = 28.0;  // padding around the locked-on star
const LINE_W:     f32 = 3.0;   // bracket line thickness
const CORNER_ARM: f32 = 18.0;  // length of each bracket arm in pixels
const BLINK_HZ:   f32 = 7.0;   // flashes per second during confirmation

const COLOR: Color = Color::new(0.2, 1.0, 0.4, 1.0);

#[derive(Copy, Clone)]
pub struct StarIdentify {
    pub elapsed:       f32,
    pub selected_star: usize,
    pub round:    u8,
}

impl StarIdentify {
    pub fn new(selected_star: usize, round: u8) -> Self {
        StarIdentify { elapsed: 0.0, selected_star, round }
    }

    pub fn update(self, world: &mut World) -> GameScene {
        let elapsed = self.elapsed + get_frame_time();
        if elapsed >= TOTAL_END {
            GameScene::StarAnalysis(StarAnalysis::new(self.selected_star, world, self.round))
        } else {
            GameScene::StarIdentify(StarIdentify { elapsed, ..self })
        }
    }

    pub fn draw(&self, world: &World) {
        let t  = self.elapsed;
        let sw = screen_width();
        let sh = screen_height();

        let star = generate_star_data(world.seed, self.selected_star, self.round);
        let cx = star.position.0 * sw;
        let cy = star.position.1 * sh;

        let (tx, ty, tw, th) = (cx - BOX_PAD, cy - BOX_PAD, BOX_PAD * 2.0, BOX_PAD * 2.0);

        // Phase 1: ease the corner-bracket box in from full-screen to the star.
        let (bx, by, bw, bh) = if t < NARROW_END {
            let p = ease_out_cubic(t / NARROW_END);
            (
                lerp(-LINE_W,           tx, p),
                lerp(-LINE_W,           ty, p),
                lerp(sw + LINE_W * 2.0, tw, p),
                lerp(sh + LINE_W * 2.0, th, p),
            )
        } else {
            (tx, ty, tw, th)
        };

        // Phase 2: blink to confirm lock-in.
        let visible = if t < NARROW_END {
            true
        } else {
            (((t - NARROW_END) * BLINK_HZ) as u32) % 2 == 0
        };

        if visible {
            draw_corner_box(bx, by, bw, bh, CORNER_ARM, LINE_W, COLOR);
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

fn ease_out_cubic(t: f32) -> f32 { 1.0 - (1.0 - t).powi(3) }

/// Draws four L-shaped corner brackets that together define a rectangle.
fn draw_corner_box(x: f32, y: f32, w: f32, h: f32, arm: f32, thickness: f32, color: Color) {
    let arm = arm.min(w * 0.5).min(h * 0.5);
    let r = x + w;
    let b = y + h;
    // top-left
    draw_line(x, y,     x + arm, y,       thickness, color);
    draw_line(x, y,     x,       y + arm, thickness, color);
    // top-right
    draw_line(r, y,     r - arm, y,       thickness, color);
    draw_line(r, y,     r,       y + arm, thickness, color);
    // bottom-left
    draw_line(x, b,     x + arm, b,       thickness, color);
    draw_line(x, b,     x,       b - arm, thickness, color);
    // bottom-right
    draw_line(r, b,     r - arm, b,       thickness, color);
    draw_line(r, b,     r,       b - arm, thickness, color);
}
