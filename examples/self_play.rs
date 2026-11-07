//! Generate complete replayable JSONL games without a graphical frontend.

use flate2::{Compression, write::GzEncoder};
use rayon::prelude::*;
use schafkopf_logic::{
    game::PlayerId,
    recording::CompactDatasetGame,
    self_play::{self, BenchmarkReport, DatasetGame},
};
use sha2::Digest;
use std::{
    error::Error,
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::PathBuf,
};

type RunError = Box<dyn Error + Send + Sync>;

struct Args {
    games: u64,
    seed: u64,
    output: PathBuf,
    compare: bool,
    threads: usize,
    compact: bool,
    shard_games: Option<u64>,
}

fn args() -> Result<Option<Args>, String> {
    let mut result = Args {
        games: 100,
        seed: 1,
        output: "data/self-play.jsonl".into(),
        compare: false,
        compact: true,
        shard_games: None,
        threads: std::thread::available_parallelism().map_or(1, usize::from),
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--games" => {
                result.games = args
                    .next()
                    .ok_or("--games needs a number")?
                    .parse()
                    .map_err(|_| "invalid --games")?
            }
            "--seed" => {
                result.seed = args
                    .next()
                    .ok_or("--seed needs a number")?
                    .parse()
                    .map_err(|_| "invalid --seed")?
            }
            "--recording" => {
                result.compact = match args.next().as_deref() {
                    Some("compact") => true,
                    Some("full") => false,
                    _ => return Err("--recording must be compact or full".into()),
                };
            }
            "--shard-games" => {
                let count = args
                    .next()
                    .ok_or("--shard-games needs a number")?
                    .parse::<u64>()
                    .map_err(|_| "invalid --shard-games")?;
                if count == 0 {
                    return Err("--shard-games must be greater than zero".into());
                }
                result.shard_games = Some(count);
            }
            "--threads" => {
                result.threads = args
                    .next()
                    .ok_or("--threads needs a number")?
                    .parse()
                    .map_err(|_| "invalid --threads")?;
            }
            "--output" => result.output = args.next().ok_or("--output needs a path")?.into(),
            "--compare" => result.compare = true,
            "--help" | "-h" => {
                println!(
                    "self_play [--games N] [--seed N] [--output PATH] [--compare] [--threads N] [--recording compact|full] [--shard-games N]\nDefaults: compact recording; 100 deals, seed 1, data/self-play.jsonl, one worker per available logical CPU.\n--threads N sets a positive worker count. Output order is independent of worker count.\nNormal: four heuristic NPCs. Compare: five games per deal; each seat compared against its first-legal baseline.\nOutput paths ending in .gz use parallel gzip compression.\n--shard-games N starts a new file every N deals (comparison variants stay together).\nA .manifest.json sidecar records provenance and completion.\nExisting output files are never overwritten. Summary is printed as JSON to stdout."
                );
                return Ok(None);
            }
            _ => return Err(format!("unknown option: {arg}")),
        }
    }
    if result.output.file_name().is_none() {
        return Err("--output must name a file".into());
    }
    if result.threads == 0 {
        return Err("--threads must be greater than zero".into());
    }
    if result.games == 0 {
        return Err("--games must be greater than zero".into());
    }
    if result.seed.checked_add(result.games - 1).is_none() {
        return Err("seed range overflows u64".into());
    }
    Ok(Some(result))
}

// Workers serialize one deal at a time; only the coordinator writes the file.
struct DealOutput {
    bytes: Vec<u8>,
    benchmark: BenchmarkReport,
    mode_counts: std::collections::BTreeMap<String, u64>,
    written: u64,
}

fn simulate_deal(args: &Args, index: u64) -> Result<DealOutput, RunError> {
    let seed = args.seed + index;
    let first = PlayerId::ALL[(index % 4) as usize];
    let mut benchmark = BenchmarkReport::default();
    let games = if args.compare {
        self_play::compare_deal(seed, first, &mut benchmark)?
    } else {
        vec![self_play::play_round(
            self_play::seeded_deck(seed),
            first,
            &mut self_play::heuristic_table(),
        )?]
    };
    let written = games.len() as u64;
    let mut mode_counts = std::collections::BTreeMap::new();
    let mut jsonl = Vec::new();
    for (variant, game) in games.into_iter().enumerate() {
        *mode_counts
            .entry(format!("{:?}", game.round.config.unwrap().mode))
            .or_default() += 1;
        let variant = if !args.compare {
            "heuristic_self_play".into()
        } else if variant == 0 {
            "first_legal_baseline".into()
        } else {
            format!("heuristic_seat_{}", variant - 1)
        };
        let record = DatasetGame {
            dataset_version: 1,
            deal_index: index,
            seed,
            variant,
            game,
        };
        if args.compact {
            serde_json::to_writer(&mut jsonl, &CompactDatasetGame::from_full(&record)?)?;
        } else {
            serde_json::to_writer(&mut jsonl, &record)?;
        }
        jsonl.push(b'\n');
    }
    // Independent gzip members keep compression on the workers and allow
    // ordered concatenation without retaining the entire dataset in memory.
    let bytes = if args
        .output
        .extension()
        .is_some_and(|extension| extension == "gz")
    {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&jsonl)?;
        encoder.finish()?
    } else {
        jsonl
    };
    Ok(DealOutput {
        bytes,
        benchmark,
        mode_counts,
        written,
    })
}

fn output_path(args: &Args, shard: u64) -> PathBuf {
    if args.shard_games.is_none() {
        return args.output.clone();
    }
    let name = args.output.file_name().unwrap().to_string_lossy();
    let suffix = if name.ends_with(".jsonl.gz") {
        ".jsonl.gz"
    } else if name.ends_with(".gz") {
        ".gz"
    } else if name.ends_with(".jsonl") {
        ".jsonl"
    } else {
        ""
    };
    args.output.with_file_name(format!(
        "{}-{shard:05}{suffix}",
        &name[..name.len() - suffix.len()]
    ))
}

fn open_output(path: &std::path::Path) -> std::io::Result<BufWriter<std::fs::File>> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(BufWriter::new)
}

fn main() -> Result<(), RunError> {
    let Some(args) = args().map_err(std::io::Error::other)? else {
        return Ok(());
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build()?;
    if let Some(parent) = args
        .output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    if output_path(&args, 0).exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "output file already exists",
        )
        .into());
    }
    let manifest_path = PathBuf::from(format!("{}.manifest.json", args.output.display()));
    // Claim the run before creating shards; never overwrite an existing run.
    let mut manifest_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&manifest_path)?;
    let git = |arguments: &[&str]| -> Option<String> {
        let output = std::process::Command::new("git")
            .args(arguments)
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    let revision = git(&["rev-parse", "HEAD"]);
    let status = git(&["status", "--porcelain", "--untracked-files=normal"]);
    let run_id = format!("{:032x}", rand::random::<u128>());
    let mut manifest = serde_json::json!({
        "manifest_version": 1, "run_id": run_id,
        "started_unix_seconds": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
        "engine_revision": revision, "engine_dirty": status.as_ref().map(|s| !s.is_empty()),
        "engine_status": status,
        "engine_package_version": env!("CARGO_PKG_VERSION"),
        "generator_binary_sha256": schafkopf_logic::recording::digest(&std::fs::read(std::env::current_exe()?)?),
        "rules": schafkopf_logic::round::RULES,
        "dataset_version": if args.compact { 2 } else { 1 },
        "recording": if args.compact { "compact" } else { "full" },
        "arguments": std::env::args().skip(1).collect::<Vec<_>>(),
        "seed": args.seed, "deals": args.games, "threads": args.threads,
        "compare": args.compare, "shard_deals": args.shard_games,
        "output": args.output, "complete": false, "shards": [],
    });
    serde_json::to_writer_pretty(&mut manifest_file, &manifest)?;
    manifest_file.flush()?;
    let mut shard_index = 0;
    let mut shard_path = output_path(&args, shard_index);
    let mut writer = open_output(&shard_path)?;
    let mut shard_hash = sha2::Sha256::new();
    let mut shard_bytes = 0u64;
    let mut shards = Vec::new();
    let mut benchmark = BenchmarkReport::default();
    let mut mode_counts = std::collections::BTreeMap::<String, u64>::new();
    let mut written = 0;
    // Bound retained records even for very large datasets. Indexed collection
    // preserves deal order regardless of the order workers finish.
    let batch_size = args.threads.saturating_mul(2).max(1) as u64;
    let mut start = 0;
    while start < args.games {
        let end = start + batch_size.min(args.games - start);
        let indices: Vec<_> = (start..end).collect();
        let results: Vec<_> = pool.install(|| {
            indices
                .into_par_iter()
                .map(|index| simulate_deal(&args, index))
                .collect()
        });
        for (offset, result) in results.into_iter().enumerate() {
            let deal = result?;
            let index = start + offset as u64;
            if args
                .shard_games
                .is_some_and(|limit| index > 0 && index % limit == 0)
            {
                writer.flush()?;
                shards.push(serde_json::json!({ "path": shard_path, "bytes": shard_bytes, "sha256": format!("{:x}", shard_hash.finalize()) }));
                shard_index += 1;
                shard_path = output_path(&args, shard_index);
                writer = open_output(&shard_path)?;
                shard_hash = sha2::Sha256::new();
                shard_bytes = 0;
            }
            writer.write_all(&deal.bytes)?;
            shard_hash.update(&deal.bytes);
            shard_bytes += deal.bytes.len() as u64;
            benchmark.merge(deal.benchmark);
            for (mode, count) in deal.mode_counts {
                *mode_counts.entry(mode).or_default() += count;
            }
            written += deal.written;
            let completed = start + offset as u64 + 1;
            if completed % 100 == 0 || completed == args.games {
                eprintln!("{} / {} deals completed", completed, args.games);
            }
        }
        start = end;
    }
    writer.flush()?;
    shards.push(serde_json::json!({ "path": shard_path, "bytes": shard_bytes, "sha256": format!("{:x}", shard_hash.finalize()) }));
    manifest["shards"] = serde_json::to_value(shards)?;
    manifest["games_written"] = written.into();
    manifest["by_mode"] = serde_json::to_value(&mode_counts)?;
    manifest["complete"] = true.into();
    use std::io::{Seek, SeekFrom};
    manifest_file.seek(SeekFrom::Start(0))?;
    manifest_file.set_len(0)?;
    serde_json::to_writer_pretty(&mut manifest_file, &manifest)?;
    manifest_file.flush()?;
    let summary = serde_json::json!({ "dataset_version": if args.compact { 2 } else { 1 }, "manifest": manifest_path, "seed": args.seed, "deals": args.games, "games_written": written, "output": args.output, "by_mode": mode_counts, "benchmark": if args.compare { Some(benchmark) } else { None } });
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concatenated_gzip_deals_decode_to_the_same_jsonl() {
        use std::io::Read;

        for compare in [false, true] {
            let mut args = Args {
                games: 3,
                seed: 42,
                output: "test.jsonl".into(),
                compare,
                threads: 1,
                compact: true,
                shard_games: None,
            };
            let mut plain = Vec::new();
            let mut compressed = Vec::new();
            for index in 0..args.games {
                args.output = "test.jsonl".into();
                plain.extend(simulate_deal(&args, index).unwrap().bytes);
                args.output = "test.jsonl.gz".into();
                compressed.extend(simulate_deal(&args, index).unwrap().bytes);
            }
            let mut decoded = Vec::new();
            flate2::read::MultiGzDecoder::new(compressed.as_slice())
                .read_to_end(&mut decoded)
                .unwrap();
            assert_eq!(decoded, plain);
            assert!(compressed.len() < plain.len());
        }
    }

    #[test]
    fn worker_count_preserves_records_and_merged_comparison_reports() {
        for compare in [false, true] {
            let args = Args {
                games: 12,
                seed: 42,
                output: PathBuf::new(),
                compare,
                threads: 1,
                compact: true,
                shard_games: None,
            };
            let run = |threads| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        (0..args.games as usize)
                            .into_par_iter()
                            .map(|index| simulate_deal(&args, index as u64).unwrap())
                            .collect::<Vec<_>>()
                    })
            };
            let serial = run(1);
            let parallel = run(4);
            let mut merged = BenchmarkReport::default();
            let mut expected = BenchmarkReport::default();
            for (index, (serial, parallel)) in serial.into_iter().zip(parallel).enumerate() {
                assert_eq!(serial.bytes, parallel.bytes);
                assert_eq!(serial.mode_counts, parallel.mode_counts);
                assert_eq!(serial.written, parallel.written);
                merged.merge(parallel.benchmark);
                if compare {
                    self_play::compare_deal(
                        args.seed + index as u64,
                        PlayerId::ALL[index % 4],
                        &mut expected,
                    )
                    .unwrap();
                }
            }
            assert_eq!(
                serde_json::to_value(merged).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
        }
    }
}
