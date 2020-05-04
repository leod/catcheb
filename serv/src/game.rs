use uuid::Uuid;

pub struct Game {
    pub settings: comn::Settings,
    pub state: comn::Game,
}

impl Game {
    pub fn new(settings: comn::Settings) -> Self {
        let state = comn::Game::new(&settings);

        Self { settings, state }
    }

    pub fn is_full(&self) -> bool {
        assert!(self.state.players.len() <= self.settings.max_num_players);
        self.state.players.len() == self.settings.max_num_players
    }

    pub fn settings(&self) -> &comn::Settings {
        &self.settings
    }

    pub fn join(&mut self, player_name: String) -> comn::PlayerId {
        // Runner takes care of not trying to join a full game.
        assert!(!self.is_full());

        let max_player_id = self
            .state
            .players
            .keys()
            .next_back()
            .cloned()
            .unwrap_or(comn::PlayerId(0));
        let player_id = comn::PlayerId(max_player_id.0 + 1);

        let player = comn::Player { name: player_name };
        self.state.players.insert(player_id, player);

        player_id
    }
}
