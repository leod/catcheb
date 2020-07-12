use std::{convert::AsRef, path::Path};

#[derive(Debug, Clone)]
pub enum LoadError {}

pub fn load_map<P: AsRef<Path>>(path: P) -> Result<comn::Map, LoadError> {
    Ok(comn::Map {
        spawn_points: Vec::new(),
        entities: Vec::new(),
        size: comn::Vector::new(320.0, 320.0),
    })
}
