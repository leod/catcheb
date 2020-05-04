use std::net::SocketAddr;

use uuid::Uuid;

use comn::util::{PingEstimation, VecOption};

pub struct Player {
    pub token: comn::PlayerToken,
    pub name: String,
    pub peer: Option<SocketAddr>,
    pub ping_estimation: PingEstimation,
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

    pub fn join(&mut self, player_name: String) -> (comn::PlayerToken, comn::PlayerId) {
        // Runner takes care of not trying to join a full game.
        assert!(!self.is_full());

        let token = comn::PlayerToken(Uuid::new_v4());
        let player = Player {
            token,
            name: player_name,
            peer: None,
            ping_estimation: PingEstimation::default(),
        };

        let player_id = self.players.add(player);

        assert!(player_id <= std::u32::MAX as usize);
        (token, comn::PlayerId(player_id as u32))
    }
}
