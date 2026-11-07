"""Analyze a recorded choice using sampled player-information states, not actual hidden hands."""
import argparse
import gzip
import json
from pathlib import Path

from ..features import RANKS, SUITS
from .bridge import Bridge


def read_game(path, index):
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(path, "rt", encoding="utf8") as file:
        if path.suffix == ".json":
            if index != 0:
                raise ValueError("a .json game has only index 0")
            return json.load(file)
        for number, line in enumerate(file):
            if number == index:
                return json.loads(line)
    raise ValueError("game index exceeds file length")


def assess(binary, record, turn=0, samples=256, seed=42, reference="heuristic", action_id=None, threads=1):
    with Bridge(binary, threads) as bridge:
        decision = bridge.call({"command": "observation", "record": record, "turn": turn})
        if action_id is not None:
            decision["action"] = {"type": "play_card", "data": {"suit": SUITS[action_id//8], "rank": RANKS[action_id%8]}}
        return bridge.call({"command": "analyze", "input": {**decision, "samples": samples, "seed": seed, "reference": reference}})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--input", type=Path, help="Completed full/compact JSONL[.gz], frontend game .json, or round .json")
    source.add_argument("--observation", type=Path, help="JSON object with observation and action; no full deal needed")
    parser.add_argument("--bridge-bin", type=Path, required=True)
    parser.add_argument("--game-index", type=int, default=0)
    parser.add_argument("--turn", type=int, default=0)
    parser.add_argument("--samples", type=int, default=256)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--reference", choices=["heuristic", "first_legal"], default="heuristic")
    parser.add_argument("--action-card", type=int)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.game_index < 0 or not 0 <= args.turn < 32 or not 8 <= args.samples <= 100_000 or not 0 <= args.seed < 2**64 or args.action_card is not None and not 0 <= args.action_card < 32:
        parser.error("invalid index, sample count, seed, or card ID")
    if args.output.exists():
        parser.error("output already exists")
    if args.observation:
        decision = json.loads(args.observation.read_text())
        if args.action_card is not None:
            decision["action"] = {"type": "play_card", "data": {"suit": SUITS[args.action_card//8], "rank": RANKS[args.action_card%8]}}
        with Bridge(args.bridge_bin, 1) as bridge:
            report = bridge.call({"command": "analyze", "input": {**decision, "samples": args.samples, "seed": args.seed, "reference": args.reference}})
    else:
        report = assess(args.bridge_bin, read_game(args.input, args.game_index), args.turn,
                        args.samples, args.seed, args.reference, args.action_card)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x") as output:
        json.dump(report, output, indent=2, allow_nan=False)
        output.write("\n")
    print(json.dumps({"report": str(args.output), "assessment": report["assessment"],
                      "estimated_decision_loss": report["estimated_decision_loss"],
                      "gain_over_reference": report["gain_over_reference"],
                      "limitations": report["limitations"]}, indent=2))


if __name__ == "__main__":
    main()
