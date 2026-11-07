"""Stream Rust-validated records into bounded, versioned training shards."""
import argparse
from collections import Counter
from contextlib import closing
import gzip
import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile

from .features import BLOCKS, FEATURE_VERSION, INPUT_SIZE, card_id, encode, mode_id, split_for


def replay_games(binary, inputs):
    process = subprocess.Popen([str(binary), *map(str, inputs)], stdout=subprocess.PIPE, text=True)
    try:
        for line in process.stdout:
            yield json.loads(line)
        if process.wait() != 0:
            raise ValueError("Rust replay validation failed; dataset is incomplete")
    finally:
        process.stdout.close()
        if process.poll() is None:
            process.terminate()
        process.wait()


class Shards:
    def __init__(self, output, max_rows):
        import pyarrow as pa
        self.pa = pa
        self.output, self.max_rows = output, max_rows
        self.schema = pa.schema([
            ("deal_id", pa.string()), ("game_id", pa.string()), ("turn", pa.uint8()),
            ("player", pa.uint8()), ("mode", pa.uint8()),
            ("features", pa.list_(pa.float32(), INPUT_SIZE)),
            ("legal", pa.list_(pa.bool_(), 32)), ("action", pa.uint8()),
            ("reward", pa.int8()),
        ], metadata={b"feature_version": str(FEATURE_VERSION).encode()})
        self.states = {}
        self.files = []

    def append(self, split, row):
        state = self.states.setdefault(split, {"buffer": [], "rows": 0, "index": 0, "writer": None})
        state["buffer"].append(row)
        if len(state["buffer"]) >= min(4096, self.max_rows - state["rows"]):
            self.flush(split)

    def flush(self, split):
        import pyarrow.parquet as pq
        state = self.states[split]
        if not state["buffer"]:
            return
        if state["writer"] is None:
            path = self.output / f'{split}-{state["index"]:05}.parquet'
            state["writer"] = pq.ParquetWriter(path, self.schema, compression="zstd")
            self.files.append(path.name)
        table = self.pa.Table.from_pylist(state["buffer"], schema=self.schema)
        state["writer"].write_table(table)
        state["rows"] += len(state["buffer"])
        state["buffer"].clear()
        if state["rows"] >= self.max_rows:
            state["writer"].close()
            state.update(writer=None, rows=0, index=state["index"] + 1)

    def close(self):
        for split, state in self.states.items():
            self.flush(split)
            if state["writer"] is not None:
                state["writer"].close()
                state["writer"] = None


def verify_manifest(path, inputs):
    manifest = json.loads(path.read_text())
    if manifest.get("manifest_version") != 1 or not manifest.get("complete"):
        raise ValueError("incomplete or unsupported collection manifest")
    # Paths inside manifests describe the original collection location. Match
    # by shard filename so a whole run can move to another directory/machine.
    shards = {Path(s["path"]).name: s for s in manifest["shards"]}
    for source in inputs:
        shard = shards.get(source.name)
        if shard is None or source.stat().st_size != shard["bytes"]:
            raise ValueError(f"unlisted or incorrect-size shard: {source}")
        checksum = hashlib.sha256()
        with source.open("rb") as file:
            while chunk := file.read(1024 * 1024):
                checksum.update(chunk)
        if checksum.hexdigest() != shard["sha256"]:
            raise ValueError(f"checksum mismatch: {source}")
    return manifest


def prepare(inputs, binary, output=None, max_games=None, shard_rows=100_000, collection_manifest=None):
    provenance = verify_manifest(collection_manifest, inputs) if collection_manifest else None
    if output:
        output.mkdir(parents=True, exist_ok=False)
    report = {"dataset_schema": 1, "feature_version": FEATURE_VERSION, "input_size": INPUT_SIZE,
              "feature_blocks": BLOCKS, "split_rule": "sha256(canonical_deal_id) mod 1000: 800/100/100",
              "inputs": [{"path": str(p.resolve()), "bytes": p.stat().st_size} for p in inputs],
              "replay_binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "collection_manifest": provenance,
              "games_read": 0, "unique_games": 0, "duplicate_games": 0,
              "by_mode": Counter(), "split_games": Counter(), "split_decisions": Counter()}
    shards = Shards(output, shard_rows) if output else None
    bids = gzip.open(output / "declarations.jsonl.gz", "xt", encoding="utf8") if output else None
    try:
        with tempfile.TemporaryDirectory(prefix="schafkopf-audit-") as tmp:
            with closing(sqlite3.connect(str(Path(tmp) / "ids.sqlite"))) as seen:
                seen.execute("CREATE TABLE games (id TEXT PRIMARY KEY)")
                seen.execute("CREATE TABLE deals (id TEXT PRIMARY KEY)")
                stream = replay_games(binary, inputs)
                try:
                    for game in stream:
                        report["games_read"] += 1
                        if game.get("replay_version") != 1 or len(game["decisions"]) != 32:
                            raise ValueError("unsupported or incomplete replay")
                        inserted = seen.execute("INSERT OR IGNORE INTO games VALUES (?)", (game["game_id"],)).rowcount
                        if not inserted:
                            report["duplicate_games"] += 1
                        else:
                            seen.execute("INSERT OR IGNORE INTO deals VALUES (?)", (game["deal_id"],))
                            report["unique_games"] += 1
                            split = split_for(game["deal_id"])
                            report["split_games"][split] += 1
                            mode = mode_id(game["decisions"][0]["observation"]["config"]["mode"])
                            report["by_mode"][str(mode)] += 1
                            for turn, decision in enumerate(game["decisions"]):
                                view = decision["observation"]
                                features, legal = encode(view)
                                action = card_id(decision["action"]["data"])
                                if not legal[action]:
                                    raise ValueError("illegal target action")
                                report["split_decisions"][split] += 1
                                if shards:
                                    shards.append(split, {"deal_id": game["deal_id"], "game_id": game["game_id"],
                                        "turn": turn, "player": view["player"], "mode": mode,
                                        "features": features, "legal": legal, "action": action,
                                        "reward": 1 if view["player"] in game["result"]["winners"] else -1})
                            if bids:
                                for declaration in game["declarations"]:
                                    bids.write(json.dumps({"deal_id": game["deal_id"], "game_id": game["game_id"],
                                        "split": split, **declaration}, separators=(",", ":")) + "\n")
                        if report["games_read"] % 1000 == 0:
                            seen.commit()
                        if max_games and report["games_read"] >= max_games:
                            break
                finally:
                    stream.close()
                report["unique_deals"] = seen.execute("SELECT COUNT(*) FROM deals").fetchone()[0]
    finally:
        if shards:
            shards.close()
        if bids:
            bids.close()
    if not report["games_read"]:
        raise ValueError("empty input dataset")
    report["complete"] = True
    if output:
        report["files"] = shards.files
        (output / "dataset.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["validate", "prepare"])
    parser.add_argument("inputs", type=Path, nargs="+")
    parser.add_argument("--replay-bin", type=Path, required=True, help="Matching Rust replay_dataset executable")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--manifest", type=Path, help="Verify completion and shard checksums (extra streaming read)")
    parser.add_argument("--max-games", type=int)
    parser.add_argument("--shard-rows", type=int, default=100_000)
    args = parser.parse_args()
    if args.max_games is not None and args.max_games < 1 or args.shard_rows < 1:
        parser.error("counts must be positive")
    if args.command == "prepare" and not args.output:
        parser.error("prepare requires --output (a new directory)")
    if args.command == "validate" and args.output:
        parser.error("--output is only used by prepare")
    print(json.dumps(prepare(args.inputs, args.replay_bin.resolve(), args.output, args.max_games, args.shard_rows, args.manifest), indent=2))


if __name__ == "__main__":
    main()
