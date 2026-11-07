# Stateless agent protocol v1

Rust types live in `schafkopf_logic::agent`; the rules engine has no HTTP client
or server dependency. HTTP serves decisions, never authoritative state mutation.
Only the acting seat's `Observation` / `DeclarationObservation` is sent. Archives,
other hands, private ratings, and complete `Round`/`Game` state are excluded.

## Endpoints

| Endpoint | Result |
|---|---|
| `GET /info` | `AgentInfo`: protocol version, supported observation versions and rules strings, capabilities, loaded policy identity, optional feature version |
| `POST /act` | `ActRequest` → `ActResponse` (chosen action and `model_policy` reason) |
| `POST /declare` | `DeclareRequest` → `DeclareResponse` (mode or null/pass), if supported |

The starter MLP service advertises only `act`. `/declare` returns HTTP 501 with
`unsupported_capability`; it never invents a pass. Clients use local heuristic
bidding until a declaration policy is explicitly supported.

All requests/responses use `application/json`. Request fields:

- `protocol_version`: 1.
- `observation_version`: 1 (separate from archive/round schema versions).
- `rules`: exact supported `round::RULES` string.
- `request_id`: opaque identifier, unique per attempted decision.
- `game_id`: opaque session-game ID, not a private-deal hash.
- `revision`: unsigned 64-bit engine-state revision.
- `observation`: the acting player's observation.

Success responses echo `protocol_version`, `request_id`, `game_id`, and `revision`,
and add `policy` and `decision`. `policy` contains `name`, `version`, and immutable
`checkpoint_id` (SHA-256 of the exact loaded NPZ bytes in the Python service).
For `/act`, `decision` is `{ "action": <Action>, "reason": "model_policy" }`.
For `/declare`, `decision` is an optional game mode; null is a deliberate pass.

Errors are non-200 JSON with `protocol_version`, an optional nested `context`
containing the three request identifiers, `code`, and `message`. The starter
service returns 400 for invalid requests, 403 for disallowed browser origins,
404 for unknown endpoints, 413 for invalid body size, 415 for a wrong content
type, and 501 for unsupported declaration inference.

## Client lifecycle

1. Capture one private observation and its game/revision/request identifiers.
2. Fetch `/info` and verify protocol, observation version, rules, and capability.
3. Request `/act` asynchronously. Do not block a render/update thread.
4. Verify echoed identifiers and that the response policy matches `/info`.
5. Check that the captured turn is still current and the action is legal.
6. Submit it to the authoritative engine, which validates it again.

One request may be pending per game. An accepted action or declaration changes
the revision. Redeal, cancel, menu transitions, and forced contracts invalidate
pending work. Results for invalidated work are discarded, without making a
fallback move in the new game. Network callbacks never mutate the game world.

The Bevy integration starts network work when the turn begins while its existing
presentation delay runs. Responses are applied only when both are ready. The
application deadline covers the handshake plus decision. Each underlying HTTP
request also has a transport timeout. A timed-out/canceled task cannot later
apply a result; cancellation also prevents a late handshake from launching `/act`.

Normal play may fall back once to the local heuristic. Strict evaluation records
the failure and pauses that turn; redeal to retry. Invalid JSON, mismatched
versions/identifiers, changed checkpoints, illegal actions, and transport failures
follow that policy. Do not silently retry indefinitely or label fallback moves
as model moves. Record actual providers per decision and audit remote attempts.

## Deployment

The Python service binds to `127.0.0.1` by default. A native local game can call
it directly. Browser builds use asynchronous HTTP and need an explicitly allowed
frontend origin. A multiplayer server must construct authenticated seat
observations and call the agent itself. A browser's localhost refers to the
browser user's machine; published multiplayer games should use a server-side
integration rather than assuming a model service exists on every visitor's PC.

The first MLP is stateless: each observation contains its required history.
There is no registration, model-training endpoint, game discovery, or persistent
agent session. Start the service after a checkpoint save finishes. Weights stay
fixed for the server's lifetime; restart explicitly to load another checkpoint.
