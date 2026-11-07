//! Machine-readable protocol fixture for clients and cross-language tests.
use schafkopf_logic::{
    agent::{ActRequest, RequestContext},
    game::PlayerId,
    self_play,
};
fn main() {
    let game = self_play::play_round(
        self_play::seeded_deck(42),
        PlayerId::ALL[0],
        &mut self_play::heuristic_table(),
    )
    .unwrap();
    let observation = game.decisions[0].observation.clone();
    let request = ActRequest::new(
        RequestContext {
            request_id: "fixture-1".into(),
            game_id: "fixture-game".into(),
            revision: 4,
        },
        observation,
    );
    if std::env::args().any(|a| a == "--validate") {
        use std::io::Read;
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input).unwrap();
        let response: schafkopf_logic::agent::ActResponse = serde_json::from_str(&input).unwrap();
        assert!(response.matches(&request));
        assert!(
            request
                .observation
                .legal_actions
                .contains(&response.decision.action)
        );
        assert_eq!(
            response.decision.reason,
            schafkopf_logic::npc::DecisionReason::ModelPolicy
        );
    } else {
        println!("{}", serde_json::to_string(&request).unwrap());
    }
}
