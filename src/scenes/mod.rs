mod initial_fade_in;
mod star_identify;
mod star_analysis;

pub use initial_fade_in::InitialFadeIn;
pub use star_identify::StarIdentify;
pub use star_analysis::StarAnalysis;

#[derive(Copy, Clone)]
pub enum GameScene {
    InitialFadeIn(InitialFadeIn),
    StarIdentify(StarIdentify),
    StarAnalysis(StarAnalysis),
}

impl GameScene {
    pub fn update(self) -> GameScene {
        match self {
            GameScene::InitialFadeIn(s) => s.update(),
            GameScene::StarIdentify(s)  => s.update(),
            GameScene::StarAnalysis(s)  => s.update(),
        }
    }

    pub fn draw(&self) {
        match self {
            GameScene::InitialFadeIn(s) => s.draw(),
            GameScene::StarIdentify(s)  => s.draw(),
            GameScene::StarAnalysis(s)  => s.draw(),
        }
    }
}
