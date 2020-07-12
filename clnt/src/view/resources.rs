use quicksilver::{
    golem::TextureFilter,
    graphics::{FontRenderer, Graphics, Image, VectorFont},
};

pub struct Resources {
    pub ttf: VectorFont,
    pub font_small: FontRenderer,
    pub font: FontRenderer,
    pub font_large: FontRenderer,
    pub icon_dash: Image,
    pub icon_hook: Image,
    pub ground: Image,
    pub player: Image,
}

impl Resources {
    pub async fn load(gfx: &mut Graphics) -> quicksilver::Result<Self> {
        let ttf = VectorFont::load("kongtext.ttf").await?;
        let font_small = ttf.to_renderer(gfx, 9.0)?;
        let font = ttf.to_renderer(gfx, 18.0)?;
        let font_large = ttf.to_renderer(gfx, 40.0)?;
        let icon_dash = Image::load(gfx, "sprint.png").await?;
        let icon_hook = Image::load(gfx, "robot-grab.png").await?;
        let mut ground = Image::load(gfx, "ground.png").await?;
        let mut player = Image::load(gfx, "player.png").await?;

        for texture in [&mut ground, &mut player].iter() {
            texture.set_magnification(TextureFilter::Nearest)?;
            texture.set_minification(TextureFilter::Nearest)?;
        }

        Ok(Self {
            ttf,
            font_small,
            font,
            font_large,
            icon_dash,
            icon_hook,
            ground,
            player,
        })
    }
}
