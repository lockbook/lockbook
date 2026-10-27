//! Import trusted, verified-request observations exported from retained logs.
//! Dry-run by default. See server/OS_METRICS.md.
use db_rs::{View, config::Config};
use lb_rs::model::file_metadata::Owner;
use libsecp256k1::PublicKey;
use lockbook_server_lib::{guard::ServerTx, schema::ServerDb, user_os::OS};
use serde::Deserialize;
use std::{
    collections::HashSet,
    env,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    public_key: String,
    os: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if !(args.len() == 2 || (args.len() == 3 && args[2] == "--apply")) {
        return Err(
            "usage: backfill_user_os <existing ServerV6 directory> <observations.jsonl> [--apply]"
                .into(),
        );
    }
    let directory = PathBuf::from(&args[0]);
    // Do not create a fresh database or run migrations on a mistyped path.
    if !directory.is_dir()
        || !std::fs::read_dir(&directory)?.any(|entry| {
            entry.is_ok_and(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                name.starts_with("db.") && name.ends_with(".log")
            })
        })
    {
        return Err("expected an existing ServerV6 database directory containing db.*.log".into());
    }

    // Validate the entire input before opening a write transaction.
    let mut observations = HashSet::new();
    for (line_no, line) in BufReader::new(File::open(&args[1])?).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let parse = || -> Result<_, Box<dyn std::error::Error>> {
            let record: Observation = serde_json::from_str(&line)?;
            let key = PublicKey::parse_slice(&base64::decode(record.public_key)?, None)?;
            let os = OS::from_header(Some(&record.os));
            if os == OS::Unknown {
                return Err("expected a known OS header value".into());
            }
            Ok((Owner(key), os))
        };
        observations.insert(parse().map_err(|error| format!("line {}: {error}", line_no + 1))?);
    }

    let mut db = ServerDb::init(&Config::default().log_location(directory))?;
    let mut tx = ServerTx::begin(&mut db)?;
    let mut additions = 0;
    let mut unmatched = 0;
    for (owner, os) in observations {
        if !tx.accounts.contains_key(&owner) {
            unmatched += 1;
        } else if !tx
            .user_os
            .get(&owner)
            .is_some_and(|oses| oses.contains(&os))
        {
            additions += 1;
            if args.len() == 3 {
                tx.user_os.insert(owner, os)?;
            }
        }
    }
    tx.end()?;
    println!(
        "{}: {additions} new user/OS memberships, {unmatched} observations without an account",
        if args.len() == 3 { "Applied" } else { "Dry run" }
    );
    Ok(())
}
