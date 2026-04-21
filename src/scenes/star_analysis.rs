use macroquad::prelude::*;
use macroquad::ui::{hash, root_ui, widgets::{self, Button, Group}, Id, Ui};
use super::GameScene;
use super::initial_fade_in::InitialFadeIn;
use crate::world::World;
use crate::star::{Planet, StarData, generate_star_data,
                  MASS_MIN, MASS_MAX, MASS_STEP,
                  PERIOD_MIN, PERIOD_MAX, PERIOD_STEP,
                  ECC_MIN, ECC_MAX, ECC_STEP};
use crate::star_rendering::generate_star;
use crate::simulation::{predict_observations, star_pixel_offset, EDGE_ON};

const INTRO_DURATION: f32 = 1.0;

// ── Graph constants ───────────────────────────────────────────────────────────

const GRAPH_WINDOW_SECS: f32 = 40.0;

const BRIGHTNESS_Y_MIN: f32 =  200.0;
const BRIGHTNESS_Y_MAX: f32 =  600.0;
const REDSHIFT_Y_MIN:   f32 = -150.0;
const REDSHIFT_Y_MAX:   f32 =  150.0;
const POSITION_Y_MIN:   f32 = -200.0;
const POSITION_Y_MAX:   f32 =  200.0;

const STAR_TEX_PX:          usize = 15;
const STAR_DISPLAY_SCALE:   f32   = 16.0;
/// Multiplies the star wobble offset for visualization only — gameplay unchanged.
const VISUAL_WOBBLE_SCALE:  f32   = 1.0;

// ── Confirm sequence constants ────────────────────────────────────────────────

const ANIM_DURATION: f32 = 0.6;
const HOLD_DURATION: f32 = 1.5;
const EXIT_DURATION: f32 = 0.5;

/// Log-ratio tolerance for mass and period matching
const LOG_TOL: f32 = 0.20;
/// Absolute tolerance for eccentricity matching.
const ECC_TOL: f32 = 0.2;

#[derive(Clone, PartialEq)]
enum ConfirmPhase { Idle, Animating, Holding, Exiting }

#[derive(Clone)]
pub struct StarAnalysis {
    pub intro_progress:   f32,
    pub selected_star:    usize,
    pub planet_guesses:   Vec<Planet>,
    pub star_data:        StarData,
    pub scene_time:       f32,
    pub round:            u8,
    confirm_phase:        ConfirmPhase,
    confirm_elapsed:      f32,
    confirm_results:      Vec<bool>,
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
            round,
            confirm_phase:   ConfirmPhase::Idle,
            confirm_elapsed: 0.0,
            confirm_results: Vec::new(),
        }
    }

    pub fn update(mut self, world: &mut World) -> GameScene {
        self.intro_progress = (self.intro_progress + get_frame_time() / INTRO_DURATION).min(1.0);
        self.scene_time += get_frame_time();

        refresh_star_texture(&self.star_data, self.scene_time, world);

        // ── Confirm sequence state machine ────────────────────────────────────
        match self.confirm_phase {
            ConfirmPhase::Animating => {
                self.confirm_elapsed += get_frame_time();
                if self.confirm_elapsed >= ANIM_DURATION {
                    self.confirm_phase   = ConfirmPhase::Holding;
                    self.confirm_elapsed = 0.0;
                }
            }
            ConfirmPhase::Holding => {}  // waits for the Continue button
            ConfirmPhase::Exiting => {
                self.confirm_elapsed += get_frame_time();
                if self.confirm_elapsed >= EXIT_DURATION {
                    let new_round = self.round.saturating_add(1);
                    let new_star  = pick_star(world.seed, new_round);
                    return GameScene::InitialFadeIn(InitialFadeIn::new_returning(new_round, new_star));
                }
            }
            ConfirmPhase::Idle => {}
        }

        // ── Right panel: planet parameter editor ──────────────────────────────
        let panel_x = screen_width() * 2.0 / 3.0;
        let panel_w = screen_width() / 3.0;
        let group_w = panel_w - 26.0;
        let mut add_planet = false;

        const LABEL_H:   f32 = 17.0;
        const BTN_ROW_H: f32 = 28.0;
        const SLIDER_H:  f32 = 20.0;
        const PARAM_H:   f32 = LABEL_H + BTN_ROW_H + SLIDER_H;
        const PLANET_H:  f32 = LABEL_H + PARAM_H * 3.0;

        const ADD_WIN_H: f32 = 50.0;
        const ADD_BTN_H: f32 = 36.0;

        root_ui().push_skin(&world.ui_skin);

        let mut remove_planet: Option<usize> = None;

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
                            ui.push_skin(&world.ui_skin_heading);
                            ui.label(None, &format!("Planet {}", i + 1));
                            ui.pop_skin();
                            if ui.button(Vec2::new(group_w - 28.0, 0.0), "-") {
                                remove_planet = Some(i);
                            }
                            param_row(ui, group_w, "Planetary Mass",        &mut planet.mass,         MASS_STEP,   MASS_STEP   * 10.0, MASS_MIN,   MASS_MAX,   hash!("mass_btns",   i), hash!("mass_slider",   i));
                            param_row(ui, group_w, "Orbital Period",        &mut planet.period,       PERIOD_STEP, PERIOD_STEP * 10.0, PERIOD_MIN, PERIOD_MAX, hash!("period_btns", i), hash!("period_slider", i));
                            param_row(ui, group_w, "Orbital Eccentricity",  &mut planet.eccentricity, ECC_STEP,    ECC_STEP    * 10.0, ECC_MIN,    ECC_MAX,    hash!("ecc_btns",    i), hash!("ecc_slider",    i));
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

        // ── Confirm Planets button (left side, below star image) ──────────────
        let display_px  = STAR_TEX_PX as f32 * STAR_DISPLAY_SCALE;
        let conf_btn_w  = 160.0;
        let conf_btn_h  = 36.0;
        let conf_win_h  = conf_btn_h + 10.0;
        let conf_win_x  = screen_width() / 6.0 - conf_btn_w / 2.0;
        let conf_win_y  = screen_height() - conf_btn_h - 16.0;

        let mut confirm_clicked = false;
        let mut continue_clicked = false;
        if self.confirm_phase == ConfirmPhase::Idle {
            widgets::Window::new(3u64, vec2(conf_win_x, conf_win_y), vec2(conf_btn_w, conf_win_h))
                .titlebar(false)
                .movable(false)
                .ui(&mut *root_ui(), |ui| {
                    if Button::new("Confirm Planets").size(Vec2::new(conf_btn_w - 10.0, conf_btn_h)).ui(ui) {
                        confirm_clicked = true;
                    }
                });
        }

        root_ui().pop_skin();

        if self.confirm_phase == ConfirmPhase::Holding {
            root_ui().push_skin(&world.ui_skin_blue_btn);
            widgets::Window::new(3u64, vec2(conf_win_x, conf_win_y), vec2(conf_btn_w, conf_win_h))
                .titlebar(false)
                .movable(false)
                .ui(&mut *root_ui(), |ui| {
                    if Button::new("Continue").size(Vec2::new(conf_btn_w - 10.0, conf_btn_h)).ui(ui) {
                        continue_clicked = true;
                    }
                });
            root_ui().pop_skin();
        }

        if add_planet {
            self.planet_guesses.push(Planet { mass: 1.0, period: 10.0, eccentricity: 0.0, direction: (1.0, 0.0) });
        }
        if let Some(idx) = remove_planet {
            self.planet_guesses.remove(idx);
        }

        if confirm_clicked {
            eprintln!("=== Confirm ===");
            eprintln!("  Real planets ({}):", self.star_data.planets.len());
            for (i, p) in self.star_data.planets.iter().enumerate() {
                eprintln!("    [{}] mass={:.3}  period={:.3}  ecc={:.3}", i, p.mass, p.period, p.eccentricity);
            }
            eprintln!("  Guesses ({}):", self.planet_guesses.len());
            for (i, g) in self.planet_guesses.iter().enumerate() {
                eprintln!("    [{}] mass={:.3}  period={:.3}  ecc={:.3}", i, g.mass, g.period, g.eccentricity);
            }
            eprintln!("  Diff (guess vs best real match):");
            for (gi, guess) in self.planet_guesses.iter().enumerate() {
                if let Some(best) = self.star_data.planets.iter().min_by(|a, b| {
                    let da = (guess.mass / a.mass).ln().abs() + (guess.period / a.period).ln().abs();
                    let db = (guess.mass / b.mass).ln().abs() + (guess.period / b.period).ln().abs();
                    da.partial_cmp(&db).unwrap()
                }) {
                    let mass_pct   = (guess.mass   / best.mass   - 1.0) * 100.0;
                    let period_pct = (guess.period / best.period - 1.0) * 100.0;
                    let ecc_abs    = guess.eccentricity - best.eccentricity;
                    eprintln!("    guess[{}] vs real: mass {:+.1}%  period {:+.1}%  ecc {:+.3}",
                              gi, mass_pct, period_pct, ecc_abs);
                }
            }
            self.confirm_results = match_planets(&self.planet_guesses, &self.star_data.planets);
            eprintln!("  Results: {:?}", self.confirm_results);
            self.confirm_phase   = ConfirmPhase::Animating;
            self.confirm_elapsed = 0.0;
        }

        if continue_clicked {
            self.confirm_phase   = ConfirmPhase::Exiting;
            self.confirm_elapsed = 0.0;
        }

        GameScene::StarAnalysis(self)
    }

    pub fn draw(&self, world: &World) {
        // ── Star view (left third) ─────────────────────────────────────────────
        let display_px = STAR_TEX_PX as f32 * STAR_DISPLAY_SCALE;
        let star_x = screen_width() * (1.0 / 6.0) - display_px / 2.0;
        let star_y = screen_height() * 0.35 - display_px / 2.0;

        let redshift = predict_observations(&self.star_data.planets, self.scene_time, EDGE_ON).redshift;
        let rs_norm  = (redshift / REDSHIFT_Y_MAX.abs()).clamp(-1.0, 1.0);
        let tint = if rs_norm >= 0.0 {
            Color::new(1.0, 1.0 - rs_norm * 0.35, 1.0 - rs_norm * 0.5, 1.0)
        } else {
            let t = -rs_norm;
            Color::new(1.0 - t * 0.5, 1.0 - t * 0.2, 1.0, 1.0)
        };

        draw_rectangle(star_x, star_y, display_px, display_px, BLACK);
        draw_texture_ex(
            &world.star_tex,
            star_x, star_y,
            tint,
            DrawTextureParams {
                dest_size: Some(vec2(display_px, display_px)),
                ..Default::default()
            },
        );

        // ── Confirm result overlays ────────────────────────────────────────────
        let result_alpha = match self.confirm_phase {
            ConfirmPhase::Idle       => 0.0,
            ConfirmPhase::Animating  => (self.confirm_elapsed / ANIM_DURATION).clamp(0.0, 1.0),
            ConfirmPhase::Holding    => 1.0,
            ConfirmPhase::Exiting    => 1.0 - (self.confirm_elapsed / EXIT_DURATION).clamp(0.0, 1.0),
        };
        if result_alpha > 0.0 {
            draw_confirm_results(&self.confirm_results, result_alpha);
        }

        // ── Graphs (centre third) ──────────────────────────────────────────────
        draw_graphs(&self.star_data.planets, &self.planet_guesses, self.scene_time);
    }
}

// ─── Planet matching ──────────────────────────────────────────────────────────

fn match_planets(guesses: &[Planet], real: &[Planet]) -> Vec<bool> {
    let mut matched = vec![false; real.len()];
    let mut results = vec![false; guesses.len()];
    for (gi, guess) in guesses.iter().enumerate() {
        for (ri, truth) in real.iter().enumerate() {
            if !matched[ri] && planet_ok(guess, truth) {
                results[gi] = true;
                matched[ri] = true;
                break;
            }
        }
    }
    results
}

fn planet_ok(guess: &Planet, truth: &Planet) -> bool {
    log_ok(guess.mass,         truth.mass,         LOG_TOL)
        && log_ok(guess.period,      truth.period,      LOG_TOL)
        && abs_ok(guess.eccentricity, truth.eccentricity, ECC_TOL)
}

/// Within `tol` in fractional log-ratio space: |ln(a/b)| ≤ tol.
fn log_ok(a: f32, b: f32, tol: f32) -> bool {
    if a <= 0.0 || b <= 0.0 { return false; }
    (a / b).ln().abs() <= tol
}

fn abs_ok(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn pick_star(seed: u64, round: u8) -> usize {
    let h = seed.wrapping_add((round as u64).wrapping_mul(0x9e3779b97f4a7c15));
    (h >> 32) as usize
}

// ─── Confirm result drawing ───────────────────────────────────────────────────

fn draw_confirm_results(results: &[bool], alpha: f32) {
    if results.is_empty() { return; }

    let cx      = screen_width() / 6.0;
    let start_y = screen_height() * 0.62;
    let spacing = (screen_height() * 0.34 / results.len().max(1) as f32).min(90.0);
    let icon_r  = 28.0;

    for (i, &ok) in results.iter().enumerate() {
        let cy = start_y + i as f32 * spacing;

        let bg_col  = if ok { Color::new(0.0, 0.75, 0.2, alpha * 0.25) }
                      else  { Color::new(0.9, 0.1,  0.1, alpha * 0.25) };
        let sym_col = if ok { Color::new(0.15, 1.0, 0.3, alpha) }
                      else  { Color::new(1.0,  0.2, 0.2, alpha) };

        draw_circle(cx, cy, icon_r, bg_col);

        if ok { draw_check(cx, cy, icon_r * 0.70, sym_col); }
        else  { draw_cross(cx, cy, icon_r * 0.55, sym_col); }

        let lbl  = format!("Planet {}", i + 1);
        let dims = measure_text(&lbl, None, 28, 1.0);
        draw_text(&lbl, cx - dims.width / 2.0, cy + icon_r + 14.0, 28.0,
                  Color::new(0.85, 0.85, 0.85, alpha));
    }
}

fn draw_check(cx: f32, cy: f32, size: f32, col: Color) {
    let w = 3.5;
    // short down-left arm, long up-right arm
    draw_line(cx - size * 0.5, cy,           cx - size * 0.1, cy + size * 0.45, w, col);
    draw_line(cx - size * 0.1, cy + size * 0.45, cx + size * 0.55, cy - size * 0.45, w, col);
}

fn draw_cross(cx: f32, cy: f32, size: f32, col: Color) {
    let w = 3.5;
    draw_line(cx - size, cy - size, cx + size, cy + size, w, col);
    draw_line(cx + size, cy - size, cx - size, cy + size, w, col);
}

// ─── Star texture refresh ─────────────────────────────────────────────────────

fn refresh_star_texture(star: &StarData, time: f32, world: &mut World) {
    let raw    = star_pixel_offset(&star.planets, time, EDGE_ON);
    let offset = (raw.0 * VISUAL_WOBBLE_SCALE, raw.1 * VISUAL_WOBBLE_SCALE);
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
    const BTN_Y:    f32 = 0.0;
    const BTN_ROW_H: f32 = 20.0;
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

fn draw_graphs(real: &[Planet], guess: &[Planet], scene_time: f32) {
    let sw   = screen_width();
    let sh   = screen_height();
    let gx   = sw / 3.0;
    let gw   = sw / 3.0;
    let gh   = sh / 3.0;
    let pad  = 8.0;
    let lbl  = 32.0;

    let graphs = [
        ("Brightness", BRIGHTNESS_Y_MIN, BRIGHTNESS_Y_MAX, 0usize),
        ("Redshift",   REDSHIFT_Y_MIN,   REDSHIFT_Y_MAX,   1),
        ("Position",   POSITION_Y_MIN,   POSITION_Y_MAX,   2),
    ];

    for (i, (label, y_min, y_max, obs_idx)) in graphs.iter().enumerate() {
        let panel_y = i as f32 * gh;

        draw_rectangle(gx, panel_y, gw, gh, Color::from_rgba(18, 18, 28, 220));
        draw_line(gx, panel_y, gx, panel_y + gh, 1.0, Color::from_rgba(70, 70, 90, 255));
        if i < 2 {
            draw_line(gx, panel_y + gh, gx + gw, panel_y + gh, 1.0, Color::from_rgba(70, 70, 90, 255));
        }

        draw_text(label, gx + pad, panel_y + lbl, 28.0, Color::from_rgba(160, 160, 180, 255));

        let ix = gx + pad;
        let iy = panel_y + lbl + 4.0;
        let iw = gw - pad * 2.0;
        let ih = gh - lbl - pad - 4.0;

        let zero_frac = (0.0_f32 - y_min) / (y_max - y_min);
        let zero_y    = iy + (1.0 - zero_frac.clamp(0.0, 1.0)) * ih;
        draw_line(ix, zero_y, ix + iw, zero_y, 1.0, Color::from_rgba(50, 50, 70, 255));

        draw_graph_series(real,  scene_time, ix, iy, iw, ih, *y_min, *y_max, *obs_idx, WHITE);
        draw_graph_series(guess, scene_time, ix, iy, iw, ih, *y_min, *y_max, *obs_idx, Color::from_rgba(220, 60, 60, 255));
    }
}

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
