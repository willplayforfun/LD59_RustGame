use macroquad::prelude::Texture2D;
use macroquad::ui::Skin;
use crate::star_rendering::PsfKernel;

pub struct World {
    pub seed:       u64,
    /// Number of stars rendered in the starfield texture.  Any valid
    /// `selected_star` index must be less than this value.
    pub star_count: usize,
    pub starfield:  Texture2D,
    pub ui_skin:          Skin,
    pub ui_skin_heading:  Skin,
    pub ui_skin_blue_btn: Skin,
    /// Shared optical PSF kernel — built once, used whenever a star texture
    /// needs to be (re)generated.
    pub psf:        PsfKernel,
    /// The live star texture for the currently analysed star.  Updated every
    /// frame by StarAnalysis to animate the wobble.
    pub star_tex:   Texture2D,
}
