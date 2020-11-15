use webglee::draw::{ColVertex, TriBatch};

pub struct Stage {
    pub plain: TriBatch<ColVertex>,
}

impl Stage {
    pub fn new(ctx: &webglee::Context) -> Result<Self, webglee::Error> {
        Ok(Stage {
            plain: TriBatch::new(ctx)?,
        })
    }

    pub fn clear(&mut self) {
        self.plain.clear();
    }
}
