use macroquad::prelude::*;
use macroquad::ui::{hash, root_ui, widgets::{self, Button, Group}, Id, Ui};
use super::GameScene;
use crate::world::World;
use crate::star::{Planet, StarData, generate_star_data};
use crate::star_rendering::generate_star;
use crate::simulation::{predict_observations, star_pixel_offset, EDGE_ON};

const INTRO_DURATION: f32 = 1.0;

// ── Graph constants ───────────────────────────────────────────────────────────

/// How many seconds of history each graph displays.
const GRAPH_WINDOW_SECS: f32 = 40.0;

/// Y-axis ranges per graph — tweak to taste.
const BRIGHTNESS_Y_MIN: f32 =  200.0;
const BRIGHTNESS_Y_MAX: f32 =  600.0;
const REDSHIFT_Y_MIN:   f32 = -150.0;
const REDSHIFT_Y_MAX:   f32 =  150.0;
const POSITION_Y_MIN:   f32 = -200.0;
const POSITION_Y_MAX:   f32 =  200.0;

/// Side length of the star texture in pixels (must be odd).
const STAR_TEX_PX: usize = 15;
/// Display scale applied to the star texture when drawing it.
const STAR_DISPLAY_SCALE: f32 = 16.0;

#[derive(Clone)]
pub struct StarAnalysis {
    pub intro_progress: f32,
    pub selected_star:  usize,
    pub planet_guesses: Vec<Planet>,
    /// The true planetary system for this star (hidden from the player).
    pub star_data:      StarData,
    /// Seconds elapsed since the scene started.  Drives the simulation.
    pub scene_time:     f32,
}

impl StarAnalysis {
    pub fn new(selected_star: usize, world: &mut World, round: u8) -> Self {
        let star_data = generate_star_data(world.seed, selected_star, round);
        refresh_star_texture(&star_data, 0.0, world);
        StarAnalysis {
            intro_progress: 0.0,
            selected_star,
            planet_guesses: Vec::new(),
            star_data,
            scene_time: 0.0,
        }
    }

    pub fn update(mut self, world: &mut World) -> GameScene {
        self.intro_progress = (self.intro_progress + get_frame_time() / INTRO_DURATION).min(1.0);
        self.scene_time += get_frame_time();

        // Keep the star texture in sync with the real planetary system.
        refresh_star_texture(&self.star_data, self.scene_time, world);

        // ── Right panel: planet parameter editor ──────────────────────────────
        let panel_x = screen_width() * 2.0 / 3.0;
        let panel_w = screen_width() / 3.0;
        let group_w = panel_w - 26.0;
        let mut add_planet = false;

        // Label margin set in World::ui_skin; LABEL_H must match: font_size + 2 * margin.
        // Default macroquad font size is 13px; margin is 2.0 → LABEL_H = 13 + 4 = 17.
        const LABEL_H:   f32 = 17.0;
        const BTN_ROW_H: f32 = 28.0;
        const SLIDER_H:  f32 = 20.0;
        const PARAM_H:   f32 = LABEL_H + BTN_ROW_H + SLIDER_H;
        const PLANET_H:  f32 = LABEL_H + PARAM_H * 3.0; // title label + 3 params

        const ADD_WIN_H: f32 = 50.0;
        const ADD_BTN_H: f32 = 36.0;

        root_ui().push_skin(&world.ui_skin);

        widgets::Window::new(1u64, vec2(panel_x, 0.0), vec2(panel_w, screen_height() - ADD_WIN_H))
            .label("Planets")
            .titlebar(true)
            .movable(false)
            .ui(&mut *root_ui(), |ui| {
                let count = self.planet_guesses.len();
                for i in 0..count {
                    let planet = &mut self.planet_guesses[i];
                    Group::new(hash!("planet", i), Vec2::new(group_w, PLANET_H))
                        .ui(ui, |ui| {
                            ui.label(None, &format!("Planet {}", i + 1));
                            param_row(ui, group_w, "Mass",   &mut planet.mass,         0.1,  1.0,  0.1,  10.0, hash!("mass_btns",   i), hash!("mass_slider",   i));
                            param_row(ui, group_w, "Period", &mut planet.period,        0.5,  5.0,  1.0, 100.0, hash!("period_btns", i), hash!("period_slider", i));
                            param_row(ui, group_w, "Ecc",    &mut planet.eccentricity,  0.01, 0.1,  0.0,  0.95, hash!("ecc_btns",    i), hash!("ecc_slider",    i));
                        });
                }
            });

        widgets::Window::new(2u64, vec2(panel_x, screen_height() - ADD_WIN_H), vec2(panel_w, ADD_WIN_H))
            .titlebar(false)
            .movable(false)
            .ui(&mut *root_ui(), |ui| {
                if Button::new("+").size(Vec2::new(group_w, ADD_BTN_H)).ui(ui) {
                    add_planet = true;
                }
            });

        root_ui().pop_skin();

        if add_planet {
            self.planet_guesses.push(Planet { mass: 1.0, period: 10.0, eccentricity: 0.0, direction: (1.0, 0.0) });
        }

        GameScene::StarAnalysis(self)
    }

    pub fn draw(&self, world: &World) {
        // ── Star view (left third) ─────────────────────────────────────────────
        let display_px = STAR_TEX_PX as f32 * STAR_DISPLAY_SCALE;
        let star_x = screen_width() * (1.0 / 6.0) - display_px / 2.0;
        let star_y = screen_height() * 0.35 - display_px / 2.0;

        // Black backing so the transparent star halo reads cleanly.
        draw_rectangle(star_x, star_y, display_px, display_px, BLACK);
        draw_texture_ex(
            &world.star_tex,
            star_x, star_y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(display_px, display_px)),
                ..Default::default()
            },
        );

        // ── Graphs (centre third) ──────────────────────────────────────────────
        draw_graphs(&self.star_data.planets, &self.planet_guesses, self.scene_time);
    }
}

// ─── Star texture refresh ─────────────────────────────────────────────────────

/// Regenerates `world.star_tex` to reflect the star's position at time `t`.
fn refresh_star_texture(star: &StarData, time: f32, world: &mut World) {
    let offset = star_pixel_offset(&star.planets, time, EDGE_ON);
    let pixels = generate_star(STAR_TEX_PX, star.temperature, star.brightness, &world.psf, offset);
    world.star_tex.update(&Image {
        bytes:  pixels,
        width:  STAR_TEX_PX as u16,
        height: STAR_TEX_PX as u16,
    });
}

// ─── Widget helpers ───────────────────────────────────────────────────────────

fn param_row(ui: &mut Ui, group_w: f32, name: &str, value: &mut f32, small: f32, big: f32, min: f32, max: f32, btn_id: Id, slider_id: Id) {
    ui.label(None, &format!("{}: {:.3}", name, *value));

    const BTN_LL_X: f32 = 5.0;
    const BTN_L_X:  f32 = BTN_LL_X + 33.0;
    const BTN_R_FROM_RIGHT:  f32 = 68.0;
    const BTN_RR_FROM_RIGHT: f32 = 35.0;
    const BTN_Y:    f32 = 4.0;
    const BTN_ROW_H: f32 = 28.0;
    const SLIDER_LEFT:  f32 = BTN_L_X + 22.0 + 2.0;
    const SLIDER_RIGHT: f32 = BTN_R_FROM_RIGHT + 2.0;

    Group::new(btn_id, Vec2::new(group_w, BTN_ROW_H))
        .ui(ui, |ui| {
            if ui.button(Vec2::new(BTN_LL_X,                    BTN_Y), "<<") { *value = (*value - big  ).clamp(min, max); }
            if ui.button(Vec2::new(BTN_L_X,                     BTN_Y), "<")  { *value = (*value - small).clamp(min, max); }
            if ui.button(Vec2::new(group_w - BTN_R_FROM_RIGHT,  BTN_Y), ">")  { *value = (*value + small).clamp(min, max); }
            if ui.button(Vec2::new(group_w - BTN_RR_FROM_RIGHT, BTN_Y), ">>") { *value = (*value + big  ).clamp(min, max); }
        });

    slider_widget(ui, slider_id, value, min, max, group_w, SLIDER_LEFT, group_w - SLIDER_RIGHT - SLIDER_LEFT);
}

fn slider_widget(ui: &mut Ui, id: Id, value: &mut f32, min: f32, max: f32, total_w: f32, x_offset: f32, width: f32) {
    const SLIDER_H:    f32   = 20.0;
    const HANDLE_W:    f32   = 10.0;
    const TRACK_H:     f32   = 6.0;
    const TRACK_COL:   Color = Color::new(0.20, 0.20, 0.27, 1.0);
    const FILL_COL:    Color = Color::new(0.31, 0.47, 0.78, 1.0);
    const HANDLE_STROKE: Color = Color::new(0.47, 0.47, 0.59, 1.0);
    const HANDLE_FILL:   Color = Color::new(0.71, 0.78, 0.94, 1.0);

    let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);

    let (pos, sx) = {
        let mut canvas = ui.canvas();
        let pos      = canvas.request_space(Vec2::new(total_w, SLIDER_H));
        let sx       = pos.x + x_offset;
        let track_y  = pos.y + SLIDER_H / 2.0 - TRACK_H / 2.0;
        let handle_x = sx + t * width - HANDLE_W / 2.0;

        canvas.rect(Rect::new(sx,       track_y, width,    TRACK_H),  None,          TRACK_COL);
        canvas.rect(Rect::new(sx,       track_y, t * width, TRACK_H), None,          FILL_COL);
        canvas.rect(Rect::new(handle_x, pos.y,   HANDLE_W, SLIDER_H), HANDLE_STROKE, HANDLE_FILL);
        (pos, sx)
    };

    let (mx, my) = mouse_position();
    let in_bounds = mx >= sx && mx <= sx + width && my >= pos.y && my < pos.y + SLIDER_H;
    let dragging  = ui.get_bool(id);

    if is_mouse_button_pressed(MouseButton::Left) && in_bounds { *dragging = true; }
    if !is_mouse_button_down(MouseButton::Left)                { *dragging = false; }
    if *dragging {
        *value = (min + (mx - sx) / width * (max - min)).clamp(min, max);
    }
}

// ─── Graph rendering ──────────────────────────────────────────────────────────

/// Draws the three stacked scrolling graphs in the centre third of the screen.
///
/// `real`       — the true planetary system (white series).
/// `guess`      — the player's current guess (red series).
/// `scene_time` — current simulation time; the right edge of the graph.
fn draw_graphs(real: &[Planet], guess: &[Planet], scene_time: f32) {
    let sw   = screen_width();
    let sh   = screen_height();
    let gx   = sw / 3.0;
    let gw   = sw / 3.0;
    let gh   = sh / 3.0;
    let pad  = 8.0;
    let lbl  = 16.0; // vertical space reserved for the label at the top

    // Each entry: (label, y_min, y_max, which observable to plot)
    // Observable selector is an index:  0 = brightness, 1 = redshift, 2 = position.x
    let graphs = [
        ("Brightness", BRIGHTNESS_Y_MIN, BRIGHTNESS_Y_MAX, 0usize),
        ("Redshift",   REDSHIFT_Y_MIN,   REDSHIFT_Y_MAX,   1),
        ("Position",   POSITION_Y_MIN,   POSITION_Y_MAX,   2),
    ];

    for (i, (label, y_min, y_max, obs_idx)) in graphs.iter().enumerate() {
        let panel_y = i as f32 * gh;

        // Panel background and border
        draw_rectangle(gx, panel_y, gw, gh, Color::from_rgba(18, 18, 28, 220));
        draw_line(gx, panel_y, gx, panel_y + gh, 1.0, Color::from_rgba(70, 70, 90, 255));
        if i < 2 {
            draw_line(gx, panel_y + gh, gx + gw, panel_y + gh, 1.0, Color::from_rgba(70, 70, 90, 255));
        }

        // Label
        draw_text(label, gx + pad, panel_y + lbl, 14.0, Color::from_rgba(160, 160, 180, 255));

        // Inner plotting area
        let ix = gx + pad;
        let iy = panel_y + lbl + 4.0;
        let iw = gw - pad * 2.0;
        let ih = gh - lbl - pad - 4.0;

        // Zero / baseline reference line
        let zero_frac = (0.0_f32 - y_min) / (y_max - y_min);
        let zero_y    = iy + (1.0 - zero_frac.clamp(0.0, 1.0)) * ih;
        draw_line(ix, zero_y, ix + iw, zero_y, 1.0, Color::from_rgba(50, 50, 70, 255));

        // Data series
        draw_graph_series(real,  scene_time, ix, iy, iw, ih, *y_min, *y_max, *obs_idx, WHITE);
        draw_graph_series(guess, scene_time, ix, iy, iw, ih, *y_min, *y_max, *obs_idx, Color::from_rgba(220, 60, 60, 255));
    }
}

/// Draws one data series (polyline) for a single graph panel.
///
/// Samples `predict_observations` at one point per pixel column across the
/// time window `[scene_time - GRAPH_WINDOW_SECS, scene_time]`.
fn draw_graph_series(
    planets:    &[Planet],
    scene_time: f32,
    x: f32, y: f32, w: f32, h: f32,
    y_min: f32, y_max: f32,
    obs_idx: usize,
    color: Color,
) {
    let samples = w as usize;
    if samples < 2 { return; }

    let mut prev: Option<(f32, f32)> = None;

    for i in 0..samples {
        let t_frac = i as f32 / (samples - 1) as f32;
        let t      = scene_time - GRAPH_WINDOW_SECS + t_frac * GRAPH_WINDOW_SECS;

        let obs = predict_observations(planets, t, EDGE_ON);
        let val = match obs_idx {
            0 => obs.brightness,
            1 => obs.redshift,
            _ => obs.position.0,
        };

        let px = x + t_frac * w;
        let py = y + (1.0 - ((val - y_min) / (y_max - y_min)).clamp(0.0, 1.0)) * h;

        if let Some((px0, py0)) = prev {
            draw_line(px0, py0, px, py, 1.5, color);
        }
        prev = Some((px, py));
    }
}
