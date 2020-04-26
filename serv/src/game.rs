use uuid::Uuid;

use comn::{game, util::VecOption};

pub struct Player {
    pub token_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub max_num_players: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_num_players: 16,
        }
    }
}

pub struct Game {
    settings: Settings,
    players: VecOption<Player>,
}

impl Game {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            players: VecOption::new(),
        }
    }

    pub fn is_full(&self) -> bool {
        assert!(self.players.len() <= self.settings.max_num_players);
        self.players.len() == self.settings.max_num_players
    }

    pub fn join(&mut self, player_name: String) -> (Uuid, game::PlayerId) {
        // Runner takes care of not trying to join a full game.
        assert!(!self.is_full());

        let token_id = Uuid::new_v4();
        let player = Player {
            token_id,
            name: player_name,
        };

        let player_id = self.players.add(player);

        assert!(player_id <= std::u16::MAX as usize);
        (token_id, game::PlayerId(player_id as u16))
    }
}
