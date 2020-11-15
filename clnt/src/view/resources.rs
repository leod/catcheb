use webglee::{Context, Error};

pub struct Resources {
    /*pub ttf: VectorFont,
pub font_small: FontRenderer,
pub font: FontRenderer,
pub font_large: FontRenderer,
pub icon_dash: Image,
pub icon_hook: Image,
pub ground: Image,
pub player: Image,
pub danger_guy: Image,*/}

impl Resources {
    pub async fn load(ctx: &Context) -> Result<Self, Error> {
        /*let ttf = VectorFont::load("kongtext.ttf").await?;
        let font_small = ttf.to_renderer(gfx, 9.0)?;
        let font = ttf.to_renderer(gfx, 18.0)?;
        let font_large = ttf.to_renderer(gfx, 40.0)?;
        let icon_dash = Image::load(gfx, "sprint.png").await?;
        let icon_hook = Image::load(gfx, "robot-grab.png").await?;
        let mut ground = Image::load(gfx, "ground.png").await?;
        let mut player = Image::load(gfx, "player.png").await?;
        let mut danger_guy = Image::load(gfx, "danger_guy.png").await?;

        for texture in [&mut ground, &mut player, &mut danger_guy].iter() {
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
            danger_guy,
        })*/

        Ok(Self {})
    }
}
