mod initial_fade_in;
mod star_identify;
mod star_analysis;

pub use initial_fade_in::InitialFadeIn;
pub use star_identify::StarIdentify;
pub use star_analysis::StarAnalysis;

use crate::world::World;

#[derive(Clone)]
pub enum GameScene {
    InitialFadeIn(InitialFadeIn),
    StarIdentify(StarIdentify),
    StarAnalysis(StarAnalysis),
}

impl GameScene {
    pub fn update(self, world: &mut World) -> GameScene {
        match self {
            GameScene::InitialFadeIn(s) => s.update(world),
            GameScene::StarIdentify(s)  => s.update(world),
            GameScene::StarAnalysis(s)  => s.update(world),
        }
    }

    pub fn draw(&self, world: &World) {
        match self {
            GameScene::InitialFadeIn(s) => s.draw(world),
            GameScene::StarIdentify(s)  => s.draw(world),
            GameScene::StarAnalysis(s)  => s.draw(world),
        }
    }
}
