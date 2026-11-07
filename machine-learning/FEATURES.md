# Feature contract v1

Card IDs are `8 * suit + rank`. Suits: Eichel, Gras, Herz, Schell (0–3).
Ranks: Ass, Zehn, Koenig, Ober, Unter, Neun, Acht, Sieben (0–7).
These IDs are shared with Rust's `Card::id` / `Card::from_id`.

Mode IDs: pass=0; Sauspiel=1–4; Solo=5–8; pure Wenz=9;
suit Wenz=10–13; pure Geier=14; suit Geier=15–18; Bettel=19; Ramsch=20.
Suit ranges use the same suit order. Herz Sauspiel has a reserved ID but is illegal.

Seats are rotated so the observing actor is seat 0: `(seat - actor) mod 4`.
Targets are absolute card IDs; suits are not permuted. Features use only fields
from the acting player's recorded observation. Full deals, other hands,
terminal outcomes, policy versions, and heuristic reasons are excluded.

Concatenate the following blocks in order (471 float32 values total):

| Block | Width | Encoding |
|---|---:|---|
| Own hand | 32 | Multi-hot by card ID |
| Public cards by player | 128 | Four relative-seat blocks of 32 cards |
| Public play order | 32 | Per card: `(chronological turn + 1) / 32`, unseen=0 |
| Retained/current trick | 148 | Four chronological slots: 33-card one-hot (32=empty), then four-seat one-hot (all zero when empty) |
| Mode | 21 | One-hot mode ID |
| Declarer | 4 | Relative-seat one-hot; ignored semantically for Ramsch |
| Opening player | 4 | Relative-seat one-hot |
| Known partner | 5 | Relative-seat one-hot, unknown=4 |
| Collected points | 4 | Relative-seat order, divide by 120 |
| Tricks won | 4 | Relative-seat order, divide by 8 |
| Hand sizes | 4 | Relative-seat order, divide by 8 |
| Completed tricks | 1 | Divide by 8 |
| Public declarations | 84 | Four relative-seat blocks of 21 mode IDs, pass=0 |

A retained completed trick is already in history; count it once in public-card
and chronological-order blocks. The current-trick block still represents the
retained table exactly as the engine observation does. Diagnostic private-hand
rating scores in trusted journals are never encoded as public information.

The legal mask is a separate array of 32 booleans. It is applied to model logits
before softmax/argmax. This version encodes bid choices but not diagnostic scores.
A future encoder may use different history representations without changing the
original recordings.
