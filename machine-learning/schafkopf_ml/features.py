"""Version 1: public information plus the acting player's private observation only."""
import hashlib

FEATURE_VERSION = 1
SUITS = ("eichel", "gras", "herz", "schell")
RANKS = ("ass", "zehn", "koenig", "ober", "unter", "neun", "acht", "sieben")
BLOCKS = (
    ("hand", 32), ("played_by_relative_seat", 128), ("play_order_by_card", 32),
    ("current_trick_card_and_relative_seat", 148), ("mode", 21),
    ("relative_declarer", 4), ("relative_first_player", 4),
    ("relative_partner_or_unknown", 5), ("points", 4), ("tricks_won", 4),
    ("hand_sizes", 4), ("completed_tricks", 1), ("declarations_by_relative_seat", 84),
)
INPUT_SIZE = sum(size for _, size in BLOCKS)


def card_id(card):
    return SUITS.index(card["suit"]) * 8 + RANKS.index(card["rank"])


def mode_id(mode):
    if mode is None:
        return 0
    if isinstance(mode, str):
        return {"bettel": 19, "ramsch": 20}[mode]
    name, suit = next(iter(mode.items()))
    if name == "wenz":
        return 9 if suit is None else 10 + SUITS.index(suit)
    if name == "geier":
        return 14 if suit is None else 15 + SUITS.index(suit)
    return {"sauspiel": 1, "solo": 5}[name] + SUITS.index(suit)


def split_for(deal_id):
    # All policies, seat rotations, and repeated collections of the same deal
    # stay together. Do not split individual decision rows randomly.
    bucket = int.from_bytes(hashlib.sha256(bytes.fromhex(deal_id)).digest()[:8], "big") % 1000
    return "train" if bucket < 800 else "validation" if bucket < 900 else "test"


def one_hot(index, size):
    result = [0.0] * size
    result[index] = 1.0
    return result


def encode(observation):
    """No full deal, result, future plays, policy reason, or reward enters features."""
    player = observation["player"]
    relative = lambda seat: (seat - player) % 4
    features = [0.0] * 32
    for card in observation["hand"]:
        features[card_id(card)] = 1.0
    played = [0.0] * 128
    order = [0.0] * 32
    public = [play for trick in observation["history"] for play in trick["plays"]]
    # A retained complete trick is already in history; don't count it twice.
    trick = observation["trick"]
    if len(trick) < 4:
        public += trick
    for turn, play in enumerate(public):
        index = card_id(play["card"])
        played[relative(play["player"]) * 32 + index] = 1.0
        order[index] = (turn + 1) / 32.0
    features += played + order
    for slot in range(4):
        play = trick[slot] if slot < len(trick) else None
        features += one_hot(card_id(play["card"]) if play else 32, 33)
        features += one_hot(relative(play["player"]), 4) if play else [0.0] * 4
    features += one_hot(mode_id(observation["config"]["mode"]), 21)
    features += one_hot(relative(observation["config"]["declarer"]), 4)
    features += one_hot(relative(observation["config"]["first_player"]), 4)
    partner = observation["partner"]
    features += one_hot(relative(partner) if partner is not None else 4, 5)
    for name, scale in [("points", 120.0), ("tricks_won", 8.0), ("hand_sizes", 8.0)]:
        features += [observation[name][(player + offset) % 4] / scale for offset in range(4)]
    features += [observation["completed_tricks"] / 8.0]
    bids = [0.0] * 84
    for bid in observation.get("declarations", []):
        bids[relative(bid["player"]) * 21 + mode_id(bid["mode"])] = 1.0
    features += bids
    legal = [False] * 32
    for action in observation["legal_actions"]:
        legal[card_id(action["data"])] = True
    if len(features) != INPUT_SIZE or not any(legal):
        raise ValueError("invalid acting-player observation")
    return features, legal
