//! Stream validated full or compact games as one expanded JSON object per line.
use flate2::read::MultiGzDecoder;
use schafkopf_logic::{
    recording::{self, CompactDatasetGame},
    self_play::DatasetGame,
};
use serde::Deserialize;
use std::{
    error::Error,
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
};

#[derive(Deserialize)]
#[serde(untagged)]
enum InputGame {
    Compact(CompactDatasetGame),
    Full(DatasetGame),
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut compact_output = false;
    let mut paths = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg == "--compact" {
            compact_output = true;
        } else {
            paths.push(arg);
        }
    }
    if paths.is_empty() {
        return Err(
            "usage: replay_dataset [--compact] PATH.jsonl[.gz] ... (validated JSONL to stdout)"
                .into(),
        );
    }
    let mut output = BufWriter::new(io::stdout().lock());
    for path in paths {
        let file = File::open(&path)?;
        let mut reader: Box<dyn BufRead> = if path.ends_with(".gz") {
            Box::new(BufReader::new(MultiGzDecoder::new(file)))
        } else {
            Box::new(BufReader::new(file))
        };
        let mut line = String::new();
        let mut number = 0;
        while reader.read_line(&mut line)? > 0 {
            number += 1;
            let result = (|| -> Result<_, Box<dyn Error>> {
                let input: InputGame = serde_json::from_str(&line)?;
                let (compact, replay) = match input {
                    InputGame::Compact(game) => {
                        let replay = game.replay()?;
                        (game, replay)
                    }
                    InputGame::Full(game) => {
                        let replay = recording::replay_full(&game)?;
                        (CompactDatasetGame::from_full(&game)?, replay)
                    }
                };
                if compact_output {
                    Ok(serde_json::to_value(compact)?)
                } else {
                    Ok(serde_json::json!({ "replay_version": 1,
                        "deal_id": compact.deal_id, "game_id": compact.game_id,
                        "variant": compact.variant, "strategies": compact.strategies,
                        "declarations": replay.declarations, "decisions": replay.decisions,
                        "result": replay.round.result }))
                }
            })();
            let value = result.map_err(|e| io::Error::other(format!("{path}:{number}: {e}")))?;
            serde_json::to_writer(&mut output, &value)?;
            output.write_all(b"\n")?;
            line.clear();
        }
    }
    output.flush()?;
    Ok(())
}
