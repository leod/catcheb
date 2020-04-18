use std::collections::HashMap;

pub struct Player {
    pub name: String,    
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub max_players: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_players: 16,
        }
    }
}

pub struct Game {
    settings: Settings,
    players: Vec<Player>,
}

impl Game {
    fn new(settings: Settings) -> Self {
        Self {
            settings,
            players: Vec::new(),
        }
    }
}