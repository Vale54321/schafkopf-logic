use rayon::prelude::*;
use schafkopf_logic::{
    agent::{ActRequest, ActResponse, DeclareRequest, DeclareResponse, RequestContext},
    deck::{Card, Deck},
    evaluation::{PairedOutcome, outcome},
    game::PlayerId,
    npc::{FirstLegal, HeuristicNpc, NpcStrategy},
    recording::deal_id,
    round::{Round, RoundPhase},
    self_play::seeded_deck,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, String>;
fn policy(name: &str) -> Result<Box<dyn NpcStrategy>> {
    match name {
        "heuristic" => Ok(Box::new(HeuristicNpc)),
        "first_legal" => Ok(Box::new(FirstLegal)),
        _ => Err(format!("unknown local policy: {name}")),
    }
}
fn opponent_policy(lineup: &str, seat: PlayerId, target: PlayerId) -> Result<&'static str> {
    match lineup {
        "heuristic" => Ok("heuristic"),
        "first_legal" => Ok("first_legal"),
        "mixed" => Ok(if (seat.index() + 4 - target.index()) % 2 == 0 {
            "heuristic"
        } else {
            "first_legal"
        }),
        _ => Err(format!("unknown opponent lineup: {lineup}")),
    }
}
fn local_bid(round: &mut Round, name: &str) -> Result<()> {
    let p = round.next_bidder().ok_or("no bidder")?;
    let choice = policy(name)?.choose_declaration(&round.declaration_observation(p));
    round.declare(p, choice).map_err(|e| e.to_string())
}
fn local_play(round: &mut Round, name: &str) -> Result<()> {
    let p = round
        .observation(PlayerId::ALL[0])
        .next_player
        .ok_or("no actor")?;
    let view = round.observation(p);
    let action = if view.legal_actions.len() == 1 {
        view.legal_actions[0]
    } else {
        policy(name)?
            .choose_action(&view)
            .ok_or("policy returned no action")?
            .action
    };
    round.play(p, action).map_err(|e| e.to_string())?;
    Ok(())
}
fn finish_baseline(
    mut round: Round,
    target: PlayerId,
    baseline: &str,
    lineup: &str,
) -> Result<Round> {
    while let Some(p) = round.next_bidder() {
        local_bid(
            &mut round,
            if p == target {
                baseline
            } else {
                opponent_policy(lineup, p, target)?
            },
        )?;
    }
    while let Some(p) = round.observation(PlayerId::ALL[0]).next_player {
        local_play(
            &mut round,
            if p == target {
                baseline
            } else {
                opponent_policy(lineup, p, target)?
            },
        )?;
    }
    Ok(round)
}

#[derive(Debug, Clone, Deserialize)]
pub struct Start {
    pub seed: u64,
    pub deals: u64,
    pub offset: u64,
    pub opponent: String,
    pub baseline: String,
    pub candidate: String,
    pub scope: String,
    #[serde(default)]
    pub deal_orders: Option<Vec<Vec<u8>>>,
}
#[derive(Serialize, Clone)]
pub struct Query {
    pub id: usize,
    pub phase: String,
    pub request: Value,
}
#[derive(Deserialize)]
pub struct Reply {
    pub id: usize,
    pub response: Value,
}
struct Case {
    id: usize,
    game_id: String,
    round: Round,
    target: PlayerId,
    opponent: String,
    candidate: String,
    result: PairedOutcome,
    query: Option<Query>,
    finished: bool,
    emitted: bool,
}
impl Case {
    fn advance(&mut self) -> Result<()> {
        self.query = None;
        loop {
            let declaring = self.round.phase() == RoundPhase::Declaring;
            let next = if declaring {
                self.round.next_bidder()
            } else {
                self.round.observation(PlayerId::ALL[0]).next_player
            };
            let Some(player) = next else {
                let record = self.round.record();
                self.result.candidate = outcome(
                    record.config.ok_or("missing contract")?,
                    record.result.as_ref().ok_or("missing result")?,
                    self.target,
                );
                self.finished = true;
                return Ok(());
            };
            let local = if player != self.target {
                Some(opponent_policy(&self.opponent, player, self.target)?)
            } else if self.candidate == "external" {
                None
            } else {
                Some(self.candidate.as_str())
            };
            if let Some(name) = local {
                if declaring {
                    local_bid(&mut self.round, name)?;
                } else {
                    local_play(&mut self.round, name)?;
                }
                continue;
            }
            let context = RequestContext {
                game_id: self.game_id.clone(),
                request_id: format!(
                    "{}:{}:{}",
                    self.game_id,
                    self.round.record().declarations.len(),
                    self.round.record().plays.len()
                ),
                revision: (self.round.record().declarations.len() + self.round.record().plays.len())
                    as u64,
            };
            if declaring {
                let view = self.round.declaration_observation(player);
                if view.legal_modes.is_empty() {
                    self.round
                        .declare(player, None)
                        .map_err(|e| e.to_string())?;
                    continue;
                }
                self.query = Some(Query {
                    id: self.id,
                    phase: "declare".into(),
                    request: serde_json::to_value(DeclareRequest::new(context, view))
                        .map_err(|e| e.to_string())?,
                });
            } else {
                let view = self.round.observation(player);
                if view.legal_actions.len() == 1 {
                    self.round
                        .play(player, view.legal_actions[0])
                        .map_err(|e| e.to_string())?;
                    continue;
                }
                self.query = Some(Query {
                    id: self.id,
                    phase: "act".into(),
                    request: serde_json::to_value(ActRequest::new(context, view))
                        .map_err(|e| e.to_string())?,
                });
            }
            return Ok(());
        }
    }
    fn reply(&mut self, response: Value) -> Result<()> {
        let query = self.query.as_ref().ok_or("case has no pending request")?;
        if query.phase == "act" {
            let request: ActRequest =
                serde_json::from_value(query.request.clone()).map_err(|e| e.to_string())?;
            let response: ActResponse =
                serde_json::from_value(response).map_err(|e| e.to_string())?;
            if !response.matches(&request) {
                return Err("protocol/context mismatch".into());
            }
            if !request
                .observation
                .legal_actions
                .contains(&response.decision.action)
            {
                return Err("illegal action".into());
            }
            self.round
                .play(self.target, response.decision.action)
                .map_err(|e| e.to_string())?;
        } else {
            let request: DeclareRequest =
                serde_json::from_value(query.request.clone()).map_err(|e| e.to_string())?;
            let response: DeclareResponse =
                serde_json::from_value(response).map_err(|e| e.to_string())?;
            if !response.matches(&request) {
                return Err("protocol/context mismatch".into());
            }
            self.round
                .declare(self.target, response.decision)
                .map_err(|e| e.to_string())?;
        }
        self.advance()
    }
}

#[derive(Default)]
pub struct Arena {
    cases: Vec<Case>,
}
impl Arena {
    pub fn start(&mut self, config: Start) -> Result<Value> {
        if self.cases.iter().any(|c| !c.finished) {
            return Err("finish the previous batch first".into());
        }
        if config.deals == 0
            || config.deals > 4096
            || config.offset.checked_add(config.deals).is_none()
            || config.seed.checked_add(config.deals - 1).is_none()
        {
            return Err("invalid batch size/seed range".into());
        }
        if config
            .deal_orders
            .as_ref()
            .is_some_and(|orders| orders.len() != config.deals as usize)
        {
            return Err("deal order count mismatch".into());
        }
        policy(&config.baseline)?;
        if config.candidate != "external" {
            policy(&config.candidate)?;
        }
        if !["card-play", "full-player"].contains(&config.scope.as_str()) {
            return Err("invalid evaluation scope".into());
        }
        opponent_policy(&config.opponent, PlayerId::ALL[1], PlayerId::ALL[0])?;
        let builds: Vec<Result<Vec<Case>>> = (0..config.deals)
            .into_par_iter()
            .map(|index| {
                let seed = config.seed + index;
                let deck = if let Some(orders) = &config.deal_orders {
                    let cards: Vec<_> = orders[index as usize]
                        .iter()
                        .map(|id| Card::from_id(*id).ok_or("invalid suite card ID".to_string()))
                        .collect::<Result<_>>()?;
                    Deck::from_deal_order(cards).map_err(str::to_owned)?
                } else {
                    seeded_deck(seed)
                };
                let group = deal_id(&deck.deal_order());
                let first = PlayerId::ALL[((config.offset + index) % 4) as usize];
                let original = Round::new(deck, first).map_err(|e| e.to_string())?;
                let mut cases = Vec::new();
                for target in PlayerId::ALL {
                    let mut initial = original.clone();
                    if config.scope == "card-play" {
                        // Bidding is frozen independently of either card policy.
                        while initial.next_bidder().is_some() {
                            local_bid(&mut initial, "heuristic")?;
                        }
                    }
                    let baseline = finish_baseline(
                        initial.clone(),
                        target,
                        &config.baseline,
                        &config.opponent,
                    )?;
                    let record = baseline.record();
                    let baseline_outcome = outcome(
                        record.config.unwrap(),
                        record.result.as_ref().unwrap(),
                        target,
                    );
                    let mut case = Case {
                        id: (index * 4) as usize + target.index(),
                        game_id: format!("arena:{:032x}", rand::random::<u128>()),
                        round: initial,
                        target,
                        opponent: config.opponent.clone(),
                        candidate: config.candidate.clone(),
                        result: PairedOutcome {
                            deal_id: group.clone(),
                            seed,
                            deal_index: config.offset + index,
                            seat: target,
                            opponent: config.opponent.clone(),
                            baseline: baseline_outcome.clone(),
                            candidate: baseline_outcome,
                        },
                        query: None,
                        finished: false,
                        emitted: false,
                    };
                    case.advance()?;
                    cases.push(case);
                }
                Ok(cases)
            })
            .collect();
        self.cases = builds
            .into_iter()
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
        Ok(self.snapshot())
    }
    pub fn respond(&mut self, replies: Vec<Reply>) -> Result<Value> {
        let mut values = std::collections::BTreeMap::new();
        for reply in replies {
            if values.insert(reply.id, reply.response).is_some() {
                return Err("duplicate response ID".into());
            }
        }
        let pending: Vec<_> = self
            .cases
            .iter()
            .filter(|c| c.query.is_some() && !c.finished)
            .map(|c| c.id)
            .collect();
        if values.len() != pending.len() || pending.iter().any(|id| !values.contains_key(id)) {
            return Err("response set must match pending cases".into());
        }
        let jobs: Vec<_> = self
            .cases
            .iter_mut()
            .filter_map(|c| values.remove(&c.id).map(|value| (c, value)))
            .collect();
        let results: Vec<_> = jobs
            .into_par_iter()
            .map(|(case, value)| case.reply(value))
            .collect();
        for result in results {
            result?;
        }
        Ok(self.snapshot())
    }
    fn snapshot(&mut self) -> Value {
        let mut results = Vec::new();
        for case in &mut self.cases {
            if case.finished && !case.emitted {
                results.push(case.result.clone());
                case.emitted = true;
            }
        }
        let requests: Vec<_> = self
            .cases
            .iter()
            .filter(|c| !c.finished)
            .filter_map(|c| c.query.clone())
            .collect();
        let deals: Vec<_> = self.cases.iter().filter(|c| c.target.index() == 0).map(|c| json!({"deal_id": c.result.deal_id, "seed": c.result.seed, "index": c.result.deal_index, "deal_order": c.round.record().deal_order.iter().map(|c| c.id()).collect::<Vec<_>>() })).collect();
        json!({"requests": requests, "results": results, "deals": deals, "finished": self.cases.iter().all(|c| c.finished)})
    }
}
