use crate::Point;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pos(Point);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Angle(f32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Entity {
    Player {
        owner: PlayerId,
    },
    Bullet {
        owner: PlayerId,
    },
    Item {
        item: Item,
    },
    ItemSpawn,
    Wall,
        pos: Point,
        size: Vector,
    },
    DangerGuy {
        start_pos: Point,
        end_pos: Point,
    },
}

pub struct EntityData {
    entity: Entity,
    pos: Option<Pos>,
}

// PLAYER
// Common:
// - last input
// - owner
// - position
// - angle
//
// Client:
// - predicted position
// 
// Server:
// - position history?

// BULLET
// Common:
// hk
