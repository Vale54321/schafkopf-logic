//! Persistent JSONL engine bridge. Policies see observations; Rust owns all rules.
#[path = "evaluation_support/analysis.rs"]
mod analysis;
#[path = "evaluation_support/arena.rs"]
mod arena;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    if let Some(flag) = args.next() {
        if flag != "--threads" {
            return Err("usage: evaluation_bridge [--threads N]".into());
        }
        let n: usize = args.next().ok_or("--threads needs a number")?.parse()?;
        if n == 0 {
            return Err("threads must be positive".into());
        }
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()?;
    }
    let mut arena = arena::Arena::default();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let response = (|| -> Result<Value, String> {
            let input: Value = serde_json::from_str(&line.map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            match input["command"].as_str().ok_or("missing command")? {
                "info" => Ok(
                    json!({"protocol_version":1,"rules":schafkopf_logic::round::RULES,"utility":schafkopf_logic::evaluation::UTILITY_VERSION}),
                ),
                "start" => arena.start(
                    serde_json::from_value(input["config"].clone()).map_err(|e| e.to_string())?,
                ),
                "respond" => arena.respond(
                    serde_json::from_value(input["responses"].clone())
                        .map_err(|e| e.to_string())?,
                ),
                "observation" => {
                    let record = input["record"].clone();
                    let replay = if record.get("compact").is_some() {
                        let game: schafkopf_logic::recording::CompactDatasetGame =
                            serde_json::from_value(record).map_err(|e| e.to_string())?;
                        game.replay().map_err(|e| e.to_string())?
                    } else if record.get("dataset_version").is_some() {
                        let game: schafkopf_logic::self_play::DatasetGame =
                            serde_json::from_value(record).map_err(|e| e.to_string())?;
                        schafkopf_logic::recording::replay_full(&game).map_err(|e| e.to_string())?
                    } else {
                        // Frontend SavedGame wrapper, or a plain completed RoundRecord.
                        let round: schafkopf_logic::round::RoundRecord =
                            serde_json::from_value(record.get("round").unwrap_or(&record).clone())
                                .map_err(|e| e.to_string())?;
                        schafkopf_logic::recording::CompactRecord::from_round(&round)
                            .map_err(|e| e.to_string())?
                            .replay()
                            .map_err(|e| e.to_string())?
                    };
                    let turn = input["turn"].as_u64().ok_or("missing turn")? as usize;
                    let decision = replay.decisions.get(turn).ok_or("turn must be in 0..31")?;
                    serde_json::to_value(decision).map_err(|e| e.to_string())
                }
                "analyze" => serde_json::to_value(analysis::analyze(
                    serde_json::from_value(input["input"].clone()).map_err(|e| e.to_string())?,
                )?)
                .map_err(|e| e.to_string()),
                "validate_report" => {
                    let _: schafkopf_logic::evaluation::ArenaReport =
                        serde_json::from_value(input["report"].clone())
                            .map_err(|e| e.to_string())?;
                    Ok(json!({"valid":true}))
                }
                _ => Err("unknown command".into()),
            }
        })();
        serde_json::to_writer(
            &mut stdout,
            &match response {
                Ok(value) => json!({"ok":true,"result":value}),
                Err(error) => json!({"ok":false,"error":error}),
            },
        )?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}
