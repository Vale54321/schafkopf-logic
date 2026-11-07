//! Stateless agent protocol v1. Transport and scheduling belong to applications.
//! Send only a seat observation; the authoritative engine validates every answer.
use crate::{
    gamemode::Gamemode,
    npc::Decision,
    observation::Observation,
    round::{DeclarationObservation, RULES},
};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
pub const OBSERVATION_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestContext {
    pub request_id: String,
    pub game_id: String,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyIdentity {
    pub name: String,
    pub version: u32,
    /// Immutable identity of the loaded checkpoint, normally its SHA-256.
    pub checkpoint_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentInfo {
    pub protocol_version: u32,
    pub observation_versions: Vec<u32>,
    pub rules: Vec<String>,
    pub capabilities: Vec<String>,
    pub policy: PolicyIdentity,
    #[serde(default)]
    pub feature_version: Option<u32>,
}
impl AgentInfo {
    pub fn supports_act(&self) -> bool {
        self.protocol_version == PROTOCOL_VERSION
            && self.observation_versions.contains(&OBSERVATION_VERSION)
            && self.rules.iter().any(|r| r == RULES)
            && self.capabilities.iter().any(|c| c == "act")
            && !self.policy.name.is_empty()
            && !self.policy.checkpoint_id.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRequest<O> {
    pub protocol_version: u32,
    pub observation_version: u32,
    pub rules: String,
    #[serde(flatten)]
    pub context: RequestContext,
    pub observation: O,
}
impl<O> AgentRequest<O> {
    pub fn new(context: RequestContext, observation: O) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            observation_version: OBSERVATION_VERSION,
            rules: RULES.into(),
            context,
            observation,
        }
    }
}
pub type ActRequest = AgentRequest<Observation>;
pub type DeclareRequest = AgentRequest<DeclarationObservation>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentResponse<D> {
    pub protocol_version: u32,
    #[serde(flatten)]
    pub context: RequestContext,
    pub policy: PolicyIdentity,
    pub decision: D,
}
impl<D> AgentResponse<D> {
    pub fn matches<O>(&self, request: &AgentRequest<O>) -> bool {
        self.protocol_version == PROTOCOL_VERSION
            && request.protocol_version == PROTOCOL_VERSION
            && request.observation_version == OBSERVATION_VERSION
            && request.rules == RULES
            && self.context == request.context
            && !self.policy.name.is_empty()
            && !self.policy.checkpoint_id.is_empty()
    }
}
pub type ActResponse = AgentResponse<Decision>;
pub type DeclareResponse = AgentResponse<Option<Gamemode>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentError {
    pub protocol_version: u32,
    pub context: Option<RequestContext>,
    pub code: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn response_must_match_request_context_and_protocol() {
        let request = AgentRequest::new(
            RequestContext {
                request_id: "a".into(),
                game_id: "game".into(),
                revision: 3,
            },
            (),
        );
        let mut response = AgentResponse {
            protocol_version: PROTOCOL_VERSION,
            context: request.context.clone(),
            policy: PolicyIdentity {
                name: "test".into(),
                version: 1,
                checkpoint_id: "hash".into(),
            },
            decision: (),
        };
        assert!(response.matches(&request));
        response.context.revision += 1;
        assert!(!response.matches(&request));
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["request_id"], "a");
        assert!(json.get("context").is_none());
    }
}
