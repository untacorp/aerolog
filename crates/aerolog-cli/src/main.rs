// Copyright 2026 AeroLog Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use aerolog_core::log::reader::LogReader;
use aerolog_core::log::segment::SegmentFile;
use aerolog_core::{AeroLogConfig, AeroLogDb, SyncMode};
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "aerolog",
    author = "AeroLog Authors",
    version = "0.1.0",
    about = "Production command-line interface and diagnostic utility for AeroLog",
    long_about = "Fast, pure Rust CLI tool to inspect, read, write, and manage AeroLog databases and streams."
)]
struct Cli {
    /// Path to the AeroLog database directory.
    #[arg(short, long, default_value = "./aerolog_data", global = true)]
    db: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Store a key-value record in a collection.
    Put {
        /// Collection name (e.g. 'users', 'sensors').
        collection: String,
        /// Entity key.
        key: String,
        /// Value payload (UTF-8 text, JSON, or binary bytes).
        value: String,
    },

    /// Retrieve a value by collection and key.
    Get {
        /// Collection name.
        collection: String,
        /// Entity key.
        key: String,
    },

    /// Soft-delete a key by appending an immutable tombstone record.
    Delete {
        /// Collection name.
        collection: String,
        /// Entity key.
        key: String,
    },

    /// Scan and display all live key-value pairs in a collection.
    Scan {
        /// Collection name.
        collection: String,
    },

    /// Manage append-only event streams.
    Stream {
        #[command(subcommand)]
        command: StreamSubcommands,
    },

    /// Perform segment compaction and tombstone garbage collection.
    Compact,

    /// Inspect physical storage segments, disk usage, and record statistics.
    Inspect,

    /// Dump all decoded physical records from disk to stdout.
    Dump,

    /// Launch an interactive database shell (REPL).
    Repl,
}

#[derive(Subcommand, Debug)]
enum StreamSubcommands {
    /// Append an event payload to an append-only stream partition.
    Append {
        /// Unique stream partition identifier.
        stream_id: String,
        /// Payload content to append.
        payload: String,
    },

    /// Read the most recent entries from a stream partition.
    Read {
        /// Unique stream partition identifier.
        stream_id: String,
        /// Maximum number of recent entries to retrieve.
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },

    /// Display metadata and total entry count for a stream partition.
    Info {
        /// Unique stream partition identifier.
        stream_id: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Put {
            collection,
            key,
            value,
        }) => {
            let db = open_db(&cli.db);
            match db.put(&collection, &key, value.into_bytes()) {
                Ok(hlc) => println!("OK [HLC: {hlc}]"),
                Err(err) => eprintln!("Error: {err}"),
            }
        }
        Some(Commands::Get { collection, key }) => {
            let db = open_db(&cli.db);
            match db.get(&collection, &key) {
                Ok(Some(val)) => print_payload(&val),
                Ok(None) => {
                    eprintln!("Key not found: {collection}:{key}");
                    std::process::exit(1);
                }
                Err(err) => eprintln!("Error: {err}"),
            }
        }
        Some(Commands::Delete { collection, key }) => {
            let db = open_db(&cli.db);
            match db.delete(&collection, &key) {
                Ok(hlc) => println!("DELETED [HLC: {hlc}]"),
                Err(err) => eprintln!("Error: {err}"),
            }
        }
        Some(Commands::Scan { collection }) => {
            let db = open_db(&cli.db);
            match db.scan_collection(&collection) {
                Ok(pairs) => {
                    println!("Collection: {collection} ({} items)", pairs.len());
                    for (k, v) in pairs {
                        let text = String::from_utf8_lossy(&v);
                        println!("  {k} => {text}");
                    }
                }
                Err(err) => eprintln!("Error: {err}"),
            }
        }
        Some(Commands::Stream { command }) => {
            let db = open_db(&cli.db);
            handle_stream_command(&db, command);
        }
        Some(Commands::Compact) => {
            let db = open_db(&cli.db);
            println!("Starting background compaction on {:?}...", cli.db);
            match db.compact() {
                Ok(stats) => {
                    let before_mb = stats.bytes_before as f64 / (1024.0 * 1024.0);
                    let after_mb = stats.bytes_after as f64 / (1024.0 * 1024.0);
                    let saved_pct = if stats.bytes_before > 0 {
                        (stats.bytes_before - stats.bytes_after) as f64 / stats.bytes_before as f64
                            * 100.0
                    } else {
                        0.0
                    };
                    println!("Compaction finished successfully:");
                    println!("  Disk size before: {:.2} MB", before_mb);
                    println!("  Disk size after : {:.2} MB", after_mb);
                    println!("  Records evicted : {}", stats.records_evicted);
                    println!("  Space reclaimed : {:.1}%", saved_pct);
                }
                Err(err) => eprintln!("Compaction failed: {err}"),
            }
        }
        Some(Commands::Inspect) => {
            inspect_database(&cli.db);
        }
        Some(Commands::Dump) => {
            dump_database(&cli.db);
        }
        Some(Commands::Repl) | None => {
            run_repl(&cli.db);
        }
    }
}

fn open_db(path: &Path) -> AeroLogDb {
    let config = AeroLogConfig::new(0x4145524F)
        .with_sync_mode(SyncMode::Periodic)
        .with_max_segment_size(16 * 1024 * 1024);

    AeroLogDb::open(path, config).unwrap_or_else(|err| {
        eprintln!("Failed to open database at {:?}: {}", path, err);
        std::process::exit(1);
    })
}

fn handle_stream_command(db: &AeroLogDb, command: StreamSubcommands) {
    match command {
        StreamSubcommands::Append { stream_id, payload } => match db.get_stream(&stream_id) {
            Ok(stream) => match stream.append(payload.into_bytes()) {
                Ok((seq, hlc)) => println!("APPENDED [Seq: {seq}, HLC: {hlc}]"),
                Err(err) => eprintln!("Error appending to stream: {err}"),
            },
            Err(err) => eprintln!("Error getting stream: {err}"),
        },
        StreamSubcommands::Read { stream_id, limit } => match db.get_stream(&stream_id) {
            Ok(stream) => match stream.read_recent(limit) {
                Ok(entries) => {
                    println!("Stream: {stream_id} ({} recent entries)", entries.len());
                    for e in entries {
                        let text = String::from_utf8_lossy(&e.payload);
                        println!("  [#{:06} | HLC: {}] {}", e.sequence, e.hlc, text);
                    }
                }
                Err(err) => eprintln!("Error reading stream: {err}"),
            },
            Err(err) => eprintln!("Error: {err}"),
        },
        StreamSubcommands::Info { stream_id } => match db.get_stream(&stream_id) {
            Ok(stream) => {
                println!("Stream: {stream_id}");
                println!("  Total entries: {}", stream.total_entries());
            }
            Err(err) => eprintln!("Error: {err}"),
        },
    }
}

fn print_payload(val: &[u8]) {
    match std::str::from_utf8(val) {
        Ok(text) => println!("{text}"),
        Err(_) => {
            // Hex fallback for non-UTF8 binary data
            let hex: String = val.iter().map(|b| format!("{b:02X}")).collect();
            println!("<binary {} bytes: {}>", val.len(), hex);
        }
    }
}

fn inspect_database(path: &Path) {
    println!("Inspecting AeroLog Database at {:?}", path);
    if !path.exists() {
        eprintln!("Error: Path does not exist");
        return;
    }

    let mut segment_ids = Vec::new();
    let mut total_bytes = 0u64;

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Some(id) = SegmentFile::parse_file_id(&p) {
                segment_ids.push(id);
                if let Ok(meta) = entry.metadata() {
                    total_bytes += meta.len();
                }
            }
        }
    }

    segment_ids.sort_unstable();

    println!("Total Segments : {}", segment_ids.len());
    println!(
        "Total Disk Size: {:.2} MB ({} bytes)",
        total_bytes as f64 / (1024.0 * 1024.0),
        total_bytes
    );

    if let Some(&active_id) = segment_ids.last() {
        println!("Active Segment : data-{active_id:06}.aerolog");
    }

    for &id in &segment_ids {
        let p = path.join(SegmentFile::filename_for_id(id));
        let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        println!(
            "  - data-{id:06}.aerolog (size: {:.2} KB)",
            size as f64 / 1024.0
        );
    }
}

fn dump_database(path: &Path) {
    if !path.exists() {
        eprintln!("Error: Path {:?} does not exist", path);
        return;
    }

    let reader = LogReader::new(path);
    let mut segment_ids = Vec::new();

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Some(id) = SegmentFile::parse_file_id(&entry.path()) {
                segment_ids.push(id);
            }
        }
    }
    segment_ids.sort_unstable();

    for id in segment_ids {
        println!("=== Segment data-{id:06}.aerolog ===");
        let _ = reader.scan_segment_streaming(id, |offset, record| {
            let key_str = String::from_utf8_lossy(&record.key);
            let val_str = String::from_utf8_lossy(&record.value);
            println!(
                "  @0x{offset:08X} [{:?}] key={key_str} val={val_str} hlc={}",
                record.header.record_type, record.header.hlc
            );
            Ok(true)
        });
    }
}

fn run_repl(db_path: &Path) {
    println!("==================================================================");
    println!("             AeroLog Interactive Shell (REPL)                     ");
    println!("==================================================================");
    println!("Database: {:?}", db_path);
    println!("Type 'HELP' for commands, 'EXIT' to quit.");
    println!("------------------------------------------------------------------");

    let db = open_db(db_path);
    let stdin = io::stdin();

    loop {
        print!("aerolog> ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        if stdin.read_line(&mut line).is_err() || line.is_empty() {
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let cmd = parts[0].to_uppercase();

        match cmd.as_str() {
            "EXIT" | "QUIT" => {
                println!("Closing database...");
                let _ = db.close();
                println!("Goodbye.");
                break;
            }
            "HELP" => {
                println!("Supported Commands:");
                println!("  PUT <collection> <key> <value>    Store a key-value record");
                println!("  GET <collection> <key>            Retrieve a record");
                println!("  DELETE <collection> <key>         Soft-delete a record");
                println!("  SCAN <collection>                 List all keys in a collection");
                println!("  STREAM APPEND <id> <payload>      Append to stream partition");
                println!("  STREAM READ <id> [limit]          Read recent events from stream");
                println!("  STREAM INFO <id>                  Display stream event count");
                println!("  COMPACT                           Reclaim space and merge segments");
                println!("  INSPECT                           View segment files and disk size");
                println!("  EXIT / QUIT                       Exit the shell");
            }
            "PUT" => {
                if parts.len() < 4 {
                    println!("Usage: PUT <collection> <key> <value>");
                    continue;
                }
                let collection = parts[1];
                let key = parts[2];
                // Join the rest as the payload value
                let val_index = trimmed
                    .char_indices()
                    .filter(|&(_, c)| !c.is_whitespace())
                    .nth(3);
                let value = if let Some((idx, _)) = val_index {
                    &trimmed[idx..]
                } else {
                    parts[3]
                };

                match db.put(collection, key, value.as_bytes().to_vec()) {
                    Ok(hlc) => println!("OK [HLC: {hlc}]"),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }
            "GET" => {
                if parts.len() != 3 {
                    println!("Usage: GET <collection> <key>");
                    continue;
                }
                let collection = parts[1];
                let key = parts[2];
                match db.get(collection, key) {
                    Ok(Some(val)) => print_payload(&val),
                    Ok(None) => println!("(nil)"),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }
            "DELETE" => {
                if parts.len() != 3 {
                    println!("Usage: DELETE <collection> <key>");
                    continue;
                }
                let collection = parts[1];
                let key = parts[2];
                match db.delete(collection, key) {
                    Ok(hlc) => println!("DELETED [HLC: {hlc}]"),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }
            "SCAN" => {
                if parts.len() != 2 {
                    println!("Usage: SCAN <collection>");
                    continue;
                }
                let collection = parts[1];
                match db.scan_collection(collection) {
                    Ok(pairs) => {
                        println!("Collection '{}': {} records", collection, pairs.len());
                        for (k, v) in pairs {
                            println!("  {} => {}", k, String::from_utf8_lossy(&v));
                        }
                    }
                    Err(e) => eprintln!("Error: {e}"),
                }
            }
            "STREAM" => {
                if parts.len() < 3 {
                    println!("Usage: STREAM APPEND|READ|INFO <stream_id> [args]");
                    continue;
                }
                let action = parts[1].to_uppercase();
                let stream_id = parts[2];

                match action.as_str() {
                    "APPEND" => {
                        if parts.len() < 4 {
                            println!("Usage: STREAM APPEND <stream_id> <payload>");
                            continue;
                        }
                        let payload_text = parts[3..].join(" ");
                        match db.get_stream(stream_id) {
                            Ok(stream) => match stream.append(payload_text.into_bytes()) {
                                Ok((seq, hlc)) => println!("APPENDED [Seq: {seq}, HLC: {hlc}]"),
                                Err(e) => eprintln!("Error: {e}"),
                            },
                            Err(e) => eprintln!("Error: {e}"),
                        }
                    }
                    "READ" => {
                        let limit = parts
                            .get(3)
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(10);
                        match db.get_stream(stream_id) {
                            Ok(stream) => match stream.read_recent(limit) {
                                Ok(entries) => {
                                    println!("Stream '{}' ({} entries):", stream_id, entries.len());
                                    for e in entries {
                                        println!(
                                            "  [#{:06} | HLC: {}] {}",
                                            e.sequence,
                                            e.hlc,
                                            String::from_utf8_lossy(&e.payload)
                                        );
                                    }
                                }
                                Err(e) => eprintln!("Error: {e}"),
                            },
                            Err(e) => eprintln!("Error: {e}"),
                        }
                    }
                    "INFO" => match db.get_stream(stream_id) {
                        Ok(stream) => {
                            println!("Stream '{}': {} entries", stream_id, stream.total_entries());
                        }
                        Err(e) => eprintln!("Error: {e}"),
                    },
                    other => println!(
                        "Unknown stream action '{}'. Use APPEND, READ, or INFO",
                        other
                    ),
                }
            }
            "COMPACT" => {
                println!("Running log compaction...");
                match db.compact() {
                    Ok(stats) => {
                        let before_mb = stats.bytes_before as f64 / (1024.0 * 1024.0);
                        let after_mb = stats.bytes_after as f64 / (1024.0 * 1024.0);
                        println!(
                            "Compacted: {:.2} MB -> {:.2} MB ({} evicted)",
                            before_mb, after_mb, stats.records_evicted
                        );
                    }
                    Err(e) => eprintln!("Compaction failed: {e}"),
                }
            }
            "INSPECT" => {
                inspect_database(db_path);
            }
            unknown => {
                println!(
                    "Unknown command: '{}'. Type 'HELP' for available commands.",
                    unknown
                );
            }
        }
    }
}
