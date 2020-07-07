use quicksilver::graphics::{FontRenderer, Graphics, Image, VectorFont};

pub struct Resources {
    pub ttf: VectorFont,
    pub font_small: FontRenderer,
    pub font: FontRenderer,
    pub font_large: FontRenderer,
    pub icon_dash: Image,
}

impl Resources {
    pub async fn load(gfx: &mut Graphics) -> quicksilver::Result<Self> {
        let ttf = VectorFont::load("kongtext.ttf").await?;
        let font_small = ttf.to_renderer(gfx, 9.0)?;
        let font = ttf.to_renderer(gfx, 18.0)?;
        let font_large = ttf.to_renderer(gfx, 40.0)?;
        let icon_dash = Image::load(gfx, "/sprint.png").await?;

        Ok(Self {
            ttf,
            font_small,
            font,
            font_large,
            icon_dash,
        })
    }
}
