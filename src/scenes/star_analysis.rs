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

    pub fn update(mut self, _world: &World) -> GameScene {
        self.intro_progress = (self.intro_progress + get_frame_time() / INTRO_DURATION).min(1.0);

        let panel_x = screen_width() * 2.0 / 3.0;
        let panel_w = screen_width() / 3.0;
        let group_w = panel_w - 26.0;
        let mut add_planet = false;

        widgets::Window::new(hash!(), vec2(panel_x, 0.0), vec2(panel_w, screen_height()))
            .label("Planets")
            .titlebar(true)
            .ui(&mut *root_ui(), |ui| {
                let count = self.planet_guesses.len();
                for i in 0..count {
                    let planet = &mut self.planet_guesses[i];

                    Group::new(hash!("planet", i), Vec2::new(group_w, 240.0))
                        .ui(ui, |ui| {
                            ui.label(None, &format!("Planet {}", i + 1));
                            param_row(ui, group_w, "Mass",   &mut planet.mass,        0.1,  1.0,  0.1,  10.0, hash!("mass_btns",   i), hash!("mass_slider",   i));
                            param_row(ui, group_w, "Period", &mut planet.period,       0.5,  5.0,  1.0, 100.0, hash!("period_btns", i), hash!("period_slider", i));
                            param_row(ui, group_w, "Ecc",    &mut planet.eccentricity, 0.01, 0.1,  0.0,  0.95, hash!("ecc_btns",    i), hash!("ecc_slider",    i));
                        });
                }

                Group::new(hash!("add_planet"), Vec2::new(group_w, 44.0))
                    .ui(ui, |ui| {
                        if Button::new("+").size(Vec2::new(group_w, 36.0)).ui(ui) {
                            add_planet = true;
                        }
                    });
            });

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
    Group::new(btn_id, Vec2::new(group_w, 28.0))
        .ui(ui, |ui| {
            if ui.button(Vec2::new(5.0,            4.0), "<<") { *value = (*value - big  ).clamp(min, max); }
            if ui.button(Vec2::new(38.0,           4.0), "<")  { *value = (*value - small).clamp(min, max); }
            if ui.button(Vec2::new(group_w - 68.0, 4.0), ">")  { *value = (*value + small).clamp(min, max); }
            if ui.button(Vec2::new(group_w - 35.0, 4.0), ">>") { *value = (*value + big  ).clamp(min, max); }
        });
    let slider_x = 62.0;                        // 2px gap after "<" button (ends at 60)
    let slider_w = group_w - 70.0 - slider_x;  // 2px gap before ">" button (starts at group_w-68)
    slider_widget(ui, slider_id, value, min, max, group_w, slider_x, slider_w);
}

fn slider_widget(ui: &mut Ui, id: Id, value: &mut f32, min: f32, max: f32, total_w: f32, x_offset: f32, width: f32) {
    const H: f32 = 20.0;
    const HANDLE_W: f32 = 10.0;

    let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);

    let (pos, sx) = {
        let mut canvas = ui.canvas();
        let pos = canvas.request_space(Vec2::new(total_w, H));
        let sx = pos.x + x_offset;
        let track_y = pos.y + H / 2.0 - 3.0;
        let handle_x = sx + t * width - HANDLE_W / 2.0;

        canvas.rect(Rect::new(sx,        track_y, width,   6.0), None, Color::from_rgba(50,  50,  70,  255));
        canvas.rect(Rect::new(sx,        track_y, t*width, 6.0), None, Color::from_rgba(80,  120, 200, 255));
        canvas.rect(Rect::new(handle_x,  pos.y,   HANDLE_W, H),
            Color::from_rgba(120, 120, 150, 255),
            Color::from_rgba(180, 200, 240, 255),
        );
        (pos, sx)
    };

    let (mx, my) = mouse_position();
    let in_bounds = mx >= sx && mx <= sx + width && my >= pos.y && my < pos.y + H;
    let dragging = ui.get_bool(id);

    if is_mouse_button_pressed(MouseButton::Left) && in_bounds { *dragging = true; }
    if !is_mouse_button_down(MouseButton::Left)               { *dragging = false; }
    if *dragging {
        *value = (min + (mx - sx) / width * (max - min)).clamp(min, max);
    }
}
