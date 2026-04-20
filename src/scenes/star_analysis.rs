use macroquad::prelude::*;
use macroquad::ui::{hash, root_ui, widgets::{self, Button, Group}, Id, Ui};
use super::GameScene;
use crate::world::World;
use crate::star::Planet;

const INTRO_DURATION: f32 = 1.0;

#[derive(Clone)]
pub struct StarAnalysis {
    pub intro_progress: f32,
    pub selected_star: usize,
    pub planet_guesses: Vec<Planet>,
}

impl StarAnalysis {
    pub fn new(selected_star: usize) -> Self {
        StarAnalysis { intro_progress: 0.0, selected_star, planet_guesses: Vec::new() }
    }

    pub fn update(mut self, world: &World) -> GameScene {
        self.intro_progress = (self.intro_progress + get_frame_time() / INTRO_DURATION).min(1.0);

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

        const ADD_WIN_H:   f32 = 50.0; // window pinned to the bottom for the + button
        const ADD_BTN_H:   f32 = 36.0;

        root_ui().push_skin(&world.ui_skin);

        // Guesses window — scrolls when content overflows
        widgets::Window::new(hash!(), vec2(panel_x, 0.0), vec2(panel_w, screen_height() - ADD_WIN_H))
            .label("Planets")
            .titlebar(true)
            .ui(&mut *root_ui(), |ui| {
                let count = self.planet_guesses.len();
                for i in 0..count {
                    let planet = &mut self.planet_guesses[i];
                    Group::new(hash!("planet", i), Vec2::new(group_w, PLANET_H))
                        .ui(ui, |ui| {
                            ui.label(None, &format!("Planet {}", i + 1));
                            param_row(ui, group_w, "Mass",   &mut planet.mass,        0.1,  1.0,  0.1,  10.0, hash!("mass_btns",   i), hash!("mass_slider",   i));
                            param_row(ui, group_w, "Period", &mut planet.period,       0.5,  5.0,  1.0, 100.0, hash!("period_btns", i), hash!("period_slider", i));
                            param_row(ui, group_w, "Ecc",    &mut planet.eccentricity, 0.01, 0.1,  0.0,  0.95, hash!("ecc_btns",    i), hash!("ecc_slider",    i));
                        });
                }
            });

        // Add-planet window — fixed height, pinned to the bottom of the panel
        widgets::Window::new(hash!(), vec2(panel_x, screen_height() - ADD_WIN_H), vec2(panel_w, ADD_WIN_H))
            .titlebar(false)
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

    pub fn draw(&self) {
        // TODO: star image, brightness/redshift graphs
    }
}

fn param_row(ui: &mut Ui, group_w: f32, name: &str, value: &mut f32, small: f32, big: f32, min: f32, max: f32, btn_id: Id, slider_id: Id) {
    ui.label(None, &format!("{}: {:.3}", name, *value));

    // Button positions within the sub-group
    const BTN_LL_X: f32 = 5.0;              // "<<"
    const BTN_L_X:  f32 = BTN_LL_X + 33.0;  // "<"
    const BTN_R_FROM_RIGHT:  f32 = 68.0; // ">"  starts at group_w - this
    const BTN_RR_FROM_RIGHT: f32 = 35.0; // ">>" starts at group_w - this
    const BTN_Y: f32 = 4.0;
    const BTN_ROW_H: f32 = 28.0;
    
    // Slider sits in the gap between "<" and ">" buttons
    const SLIDER_LEFT:  f32 = BTN_L_X + 22.0 + 2.0; // 2px gap after "<"
    const SLIDER_RIGHT: f32 = BTN_R_FROM_RIGHT + 2.0;   // 2px gap before ">"

    Group::new(btn_id, Vec2::new(group_w, BTN_ROW_H))
        .ui(ui, |ui| {
            if ui.button(Vec2::new(BTN_LL_X,                   BTN_Y), "<<") { *value = (*value - big  ).clamp(min, max); }
            if ui.button(Vec2::new(BTN_L_X,                    BTN_Y), "<")  { *value = (*value - small).clamp(min, max); }
            if ui.button(Vec2::new(group_w - BTN_R_FROM_RIGHT,  BTN_Y), ">")  { *value = (*value + small).clamp(min, max); }
            if ui.button(Vec2::new(group_w - BTN_RR_FROM_RIGHT, BTN_Y), ">>") { *value = (*value + big  ).clamp(min, max); }
        });

    slider_widget(ui, slider_id, value, min, max, group_w, SLIDER_LEFT, group_w - SLIDER_RIGHT - SLIDER_LEFT);
}

fn slider_widget(ui: &mut Ui, id: Id, value: &mut f32, min: f32, max: f32, total_w: f32, x_offset: f32, width: f32) {
    const SLIDER_H:  f32 = 20.0;
    const HANDLE_W:  f32 = 10.0;
    const TRACK_H:   f32 = 6.0;
    const TRACK_COL: Color = Color::new(0.20, 0.20, 0.27, 1.0);
    const FILL_COL:  Color = Color::new(0.31, 0.47, 0.78, 1.0);
    const HANDLE_STROKE: Color = Color::new(0.47, 0.47, 0.59, 1.0);
    const HANDLE_FILL:   Color = Color::new(0.71, 0.78, 0.94, 1.0);

    let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);

    let (pos, sx) = {
        let mut canvas = ui.canvas();
        let pos      = canvas.request_space(Vec2::new(total_w, SLIDER_H));
        let sx       = pos.x + x_offset;
        let track_y  = pos.y + SLIDER_H / 2.0 - TRACK_H / 2.0;
        let handle_x = sx + t * width - HANDLE_W / 2.0;

        canvas.rect(Rect::new(sx,       track_y, width,   TRACK_H), None,         TRACK_COL);
        canvas.rect(Rect::new(sx,       track_y, t*width, TRACK_H), None,         FILL_COL);
        canvas.rect(Rect::new(handle_x, pos.y,   HANDLE_W, SLIDER_H), HANDLE_STROKE, HANDLE_FILL);
        (pos, sx)
    };

    let (mx, my) = mouse_position();
    let in_bounds = mx >= sx && mx <= sx + width && my >= pos.y && my < pos.y + SLIDER_H;
    let dragging  = ui.get_bool(id);

    if is_mouse_button_pressed(MouseButton::Left) && in_bounds { *dragging = true; }
    if !is_mouse_button_down(MouseButton::Left)               { *dragging = false; }
    if *dragging {
        *value = (min + (mx - sx) / width * (max - min)).clamp(min, max);
    }
}
