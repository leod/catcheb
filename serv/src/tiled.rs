use std::{convert::AsRef, path::Path};

use comn::{
    game::entities::{DangerGuy, FoodSpawn, Turret, Wall},
    geom::AaRect,
};

pub const PLAYER_SPAWN_NAME: &str = "spawn";

#[derive(Debug)]
pub enum LoadError {
    Tiled(tiled::TiledError),
    UnknownEntityType(String),
    MissingProperty(String),
    WrongTypeProperty(String),
}

pub fn load_map<P: AsRef<Path>>(path: P) -> Result<comn::Map, LoadError> {
    let tiled_map = tiled::parse_file(path.as_ref()).map_err(LoadError::Tiled)?;

    let size = comn::Vector::new(
        (tiled_map.width * tiled_map.tile_width) as f32,
        (tiled_map.height * tiled_map.tile_height) as f32,
    );

    let spawn_points = tiled_map
        .object_groups
        .iter()
        .flat_map(|group| {
            group.objects.iter().filter_map(|object| {
                if object_name(&object) == PLAYER_SPAWN_NAME {
                    Some(object_center(&object))
                } else {
                    None
                }
            })
        })
        .collect();

    let entities: Result<Vec<comn::Entity>, LoadError> = tiled_map
        .object_groups
        .iter()
        .flat_map(|group| {
            group
                .objects
                .iter()
                .filter(|object| object_name(&object) != PLAYER_SPAWN_NAME)
                .map(|object| object_to_entity(object))
        })
        .collect();

    Ok(comn::Map {
        spawn_points,
        entities: entities?,
        size,
    })
}

fn object_to_entity(object: &tiled::Object) -> Result<comn::Entity, LoadError> {
    let entity = match object_name(object) {
        "turret" => comn::Entity::Turret(Turret::new(object_center(object))),
        "wall" => comn::Entity::Wall(Wall {
            rect: object_aa_rect(object),
        }),
        "food_spawn" => comn::Entity::FoodSpawn(FoodSpawn::new(object_center(object))),
        "danger_guy" => comn::Entity::DangerGuy(DangerGuy {
            start_pos: object_center(object),
            end_pos: object_center(object)
                + comn::Vector::new(
                    read_property_f32(object, "delta_x")?,
                    read_property_f32(object, "delta_y")?,
                ),
            size: object_size(object),
            speed: (
                read_property_f32(object, "speed_go")?,
                read_property_f32(object, "speed_back")?,
            ),
            wait_time: (
                read_property_f32(object, "wait_go")?,
                read_property_f32(object, "wait_back")?,
            ),
            phase: read_property_f32(object, "phase")?,
            is_hot: true,
        }),
        name => {
            return Err(LoadError::UnknownEntityType(name.to_string()));
        }
    };

    Ok(entity)
}

fn read_property_f32(object: &tiled::Object, prop_key: &str) -> Result<f32, LoadError> {
    let prop_value = object
        .properties
        .get(prop_key)
        .ok_or_else(|| LoadError::MissingProperty(prop_key.to_string()))?;
    if let tiled::PropertyValue::FloatValue(result) = prop_value {
        Ok(*result)
    } else {
        Err(LoadError::WrongTypeProperty(prop_key.to_string()))
    }
}

fn object_name(object: &tiled::Object) -> &str {
    if object.obj_type.is_empty() {
        &object.name
    } else {
        &object.obj_type
    }
}

fn object_aa_rect(object: &tiled::Object) -> AaRect {
    AaRect::new_top_left(object_top_left(object), object_size(object))
}

fn object_center(object: &tiled::Object) -> comn::Point {
    object_top_left(object) + object_size(object) / 2.0
}

fn object_top_left(object: &tiled::Object) -> comn::Point {
    comn::Point::new(object.x, object.y)
}

fn object_size(object: &tiled::Object) -> comn::Vector {
    comn::Vector::new(object.width, object.height)
}
