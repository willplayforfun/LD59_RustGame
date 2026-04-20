use macroquad::prelude::Texture2D;
use macroquad::ui::Skin;

pub struct World {
    pub seed: u64,
    pub starfield: Texture2D,
    pub ui_skin: Skin,
}
