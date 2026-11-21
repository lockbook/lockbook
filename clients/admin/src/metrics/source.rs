use super::Result;
use super::history::{Entry, Frame, History, Replay, decode, invalid, read_frame};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PROJECT: &str = "lockbook-net";
const INSTANCE: &str = "lb-prod";
const ZONE: &str = "us-east4-c";

// Run over the same SSH connection path Funnel uses. The directory comes
// from argv, and the only opened payload files are numbered db-rs segments.
const REMOTE: &str = include_str!("server_logs.py");

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct Segment {
    number: u64,
    identity: String,
    length: u64,
    stamp: u64,
    first_end: u64,
}

#[derive(Deserialize)]
struct Inventory {
    segments: Vec<Segment>,
    observed: i64,
}

#[derive(Clone, Deserialize, Serialize)]
struct CachedSegment {
    source: Segment,
    skip: u64,
    #[serde(default)]
    tail: Tail,
}

#[derive(Default, Deserialize, Serialize)]
struct CacheManifest {
    remote: String,
    generation: String,
    segments: BTreeMap<u64, CachedSegment>,
}

fn number(name: &str) -> Option<u64> {
    name.strip_prefix("db.")?.strip_suffix(".log")?.parse().ok()
}

fn shell_argument(argument: &str) -> String {
    format!("'{}'", argument.replace('\'', "'\"'\"'"))
}

fn ssh(remote: &str, args: &[String]) -> Command {
    let command =
        ["python3".to_owned(), "-c".into(), shell_argument(REMOTE), shell_argument(remote)]
            .into_iter()
            .chain(args.iter().map(|arg| shell_argument(arg)))
            .collect::<Vec<_>>()
            .join(" ");
    let mut process = Command::new("gcloud");
    process.args([
        "compute",
        "ssh",
        INSTANCE,
        "--project",
        PROJECT,
        "--zone",
        ZONE,
        "--command",
        &command,
        "--ssh-flag=-T",
    ]);
    process
}

fn describe(remote: &str) -> Result<Inventory> {
    let output = ssh(remote, &["inventory".into()])
        .stderr(Stdio::inherit())
        .output()?;
    if !output.status.success() {
        return Err(format!("gcloud inventory exited with {}", output.status).into());
    }
    let inventory: Inventory = serde_json::from_slice(&output.stdout)?;
    if inventory.segments.is_empty() {
        return Err(invalid("production directory has no db-rs segments"));
    }
    if !inventory
        .segments
        .windows(2)
        .all(|pair| pair[0].number < pair[1].number)
        || inventory.segments.iter().any(|segment| {
            segment.first_end > segment.length
                || !segment
                    .identity
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b'-')
                || segment.identity.is_empty()
        })
    {
        return Err(invalid("invalid production segment inventory"));
    }
    Ok(inventory)
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
struct Tail {
    last: Option<u64>,
    closed: bool,
    partial: bool,
    offset: u64,
}

fn snapshot_prefix(path: &Path, number: u64) -> Result<Option<u64>> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let mut frame_size = [0; 8];
    if length < 8 {
        return Ok(None);
    }
    file.read_exact(&mut frame_size)?;
    let frame_size = u64::from_be_bytes(frame_size);
    if frame_size > length - 8 {
        return Ok(None);
    }
    let mut header = file.take(frame_size.min(10));
    let config = bincode::config::standard();
    let variant: u32 = bincode::serde::decode_from_std_read(&mut header, config)?;
    if variant != 0 {
        return Ok(None);
    }
    let sequence: u64 = bincode::serde::decode_from_std_read(&mut header, config)?;
    Ok((sequence == number).then_some(8 + frame_size))
}

fn scan(
    path: &Path, number: u64, skip: u64, process: impl FnMut(u64, &[u8], bool) -> Result<()>,
) -> Result<Tail> {
    scan_from(
        path,
        number,
        Tail { last: (skip > 0).then_some(number), offset: skip, ..Default::default() },
        process,
    )
}

fn scan_from(
    path: &Path, number: u64, mut tail: Tail,
    mut process: impl FnMut(u64, &[u8], bool) -> Result<()>,
) -> Result<Tail> {
    let file = File::open(path)?;
    let length = file.metadata()?.len();
    let mut reader = BufReader::new(file);
    if tail.offset > length || (tail.closed && tail.offset != length) {
        return Err(invalid(format!("cached bytes changed in segment {number}")));
    }
    reader.seek(SeekFrom::Start(tail.offset))?;
    let mut first = tail.offset == 0;
    tail.partial = false;
    loop {
        let frame = match read_frame(&mut reader, length)? {
            Frame::Complete(bytes) => bytes,
            Frame::End => break,
            Frame::Partial => {
                tail.partial = true;
                break;
            }
        };
        match decode::<Entry<'_>>(&frame)? {
            Entry::Events { seq_no, payload } => {
                let baseline = first && number != 0;
                let expected =
                    if baseline { Some(number) } else { tail.last.unwrap_or(0).checked_add(1) };
                if Some(seq_no) != expected {
                    return Err(invalid(format!(
                        "segment {number}: unexpected sequence {seq_no} after {:?}",
                        tail.last
                    )));
                }
                process(seq_no, payload, baseline)?;
                tail.last = Some(seq_no);
            }
            Entry::Snapshot => {
                tail.closed = true;
                if reader.stream_position()? != length {
                    return Err(invalid(format!(
                        "segment {number} has data after its rotation marker"
                    )));
                }
            }
        }
        tail.offset = reader.stream_position()?;
        first = false;
    }
    Ok(tail)
}

fn replay(segments: Vec<(u64, PathBuf, u64)>, observed: Option<i64>) -> Result<History> {
    if segments.is_empty() {
        return Err(invalid("no db.<sequence>.log files in this directory"));
    }
    let mut replay = Replay::default();
    replay.history.oldest = segments[0].0;
    let mut prior = None::<Tail>;
    let last_index = segments.len() - 1;
    for (index, (number, path, mut skip)) in segments.into_iter().enumerate() {
        let continuous = prior
            .as_ref()
            .is_some_and(|tail| tail.closed && tail.last == Some(number));
        if skip != 0 && !continuous {
            return Err(invalid(format!("segment {number} needs its initial snapshot")));
        }
        if continuous && skip == 0 {
            skip = snapshot_prefix(&path, number)?.ok_or_else(|| {
                invalid(format!("segment {number} has no complete snapshot header"))
            })?;
        }
        if let Some(prior) = &prior {
            if !continuous {
                let previous = prior.last.unwrap_or(replay.history.latest);
                if previous >= number {
                    return Err(invalid(format!("overlapping database segment {number}")));
                }
                replay.history.gaps.push((previous, number));
            }
        }
        let tail = scan(&path, number, skip, |seq, data, baseline| {
            if !baseline || !continuous {
                replay.apply(seq, data, baseline)?;
            }
            Ok(())
        })?;
        if index == last_index {
            replay.history.partial_tail = tail.partial || tail.closed;
        }
        if tail.last.is_none() && (number != 0 || index != last_index) {
            return Err(invalid(format!("segment {number} has no complete starting frame")));
        }
        prior = Some(tail);
    }
    if let Some(observed) = observed.filter(|_| !replay.history.partial_tail) {
        replay.history.anchor(observed, replay.history.latest);
    }
    Ok(replay.history)
}

pub fn local(path: &Path) -> Result<History> {
    let segments = local_segments(path)?;
    let nested = path.join("ServerV6");
    let segments =
        if segments.is_empty() && nested.is_dir() { local_segments(&nested)? } else { segments };
    replay(segments, None)
}

fn local_segments(path: &Path) -> Result<Vec<(u64, PathBuf, u64)>> {
    let mut segments = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            if let Some(number) = entry.file_name().to_str().and_then(number) {
                segments.push((number, entry.path(), 0));
            }
        }
    }
    segments.sort_by_key(|entry| entry.0);
    Ok(segments)
}

fn segment_path(directory: &Path, source: &Segment) -> PathBuf {
    directory.join(format!("{}-{}.range", source.number, source.identity))
}

fn read_manifest(path: &Path, remote: &str) -> Result<CacheManifest> {
    match File::open(path) {
        Ok(file) => {
            let manifest: CacheManifest = serde_json::from_reader(BufReader::new(file))?;
            if manifest.remote != remote {
                return Err("the production cache belongs to a different log directory".into());
            }
            if !manifest.generation.starts_with("ranges-")
                || !manifest
                    .generation
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            {
                return Err(invalid("invalid local metrics cache generation"));
            }
            Ok(manifest)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Ok(CacheManifest { remote: remote.to_owned(), ..Default::default() })
        }
        Err(error) => Err(error.into()),
    }
}

fn save_manifest(directory: &Path, path: &Path, manifest: &CacheManifest) -> Result<()> {
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer(&mut temporary, manifest)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

fn offset_for(segment: &Segment, contiguous: bool, existing: Option<&CachedSegment>) -> u64 {
    existing.map_or_else(
        || if segment.number != 0 && contiguous { segment.first_end } else { 0 },
        |cached| cached.skip,
    )
}

fn transfer(
    remote: &str, segment: &Segment, file: &mut File, start: u64, end: u64,
    progress: &impl Fn(String),
) -> Result<()> {
    let args = [
        "read".into(),
        segment.number.to_string(),
        segment.identity.clone(),
        segment.stamp.to_string(),
        segment.length.to_string(),
        start.to_string(),
        end.to_string(),
    ];
    progress(format!(
        "Downloading segment {} · {}",
        segment.number,
        super::format_bytes(end - start)
    ));
    let mut child = ssh(remote, &args)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let stdout = child.stdout.take().ok_or("gcloud did not open stdout")?;
    let mut stream = GzDecoder::new(stdout).take(end - start + 1);
    let copied = (|| -> io::Result<u64> {
        let mut buffer = vec![0; 256 * 1024];
        let mut copied = 0;
        let mut last_report = Instant::now();
        loop {
            let read = stream.read(&mut buffer)?;
            if read == 0 {
                return Ok(copied);
            }
            file.write_all(&buffer[..read])?;
            copied += read as u64;
            if last_report.elapsed() >= Duration::from_millis(200) {
                progress(format!(
                    "Downloading segment {} · {} of {}",
                    segment.number,
                    super::format_bytes(copied),
                    super::format_bytes(end - start)
                ));
                last_report = Instant::now();
            }
        }
    })();
    if copied.is_err() {
        let _ = child.kill();
    }
    let status = child.wait()?;
    file.flush()?;
    file.sync_all()?;
    if !status.success() || copied? != end - start {
        return Err(format!("incomplete download of segment {} ({status})", segment.number).into());
    }
    Ok(())
}

pub fn refresh(
    directory: &Path, remote: &str, progress: impl Fn(String),
) -> Result<(History, u64)> {
    super::private_directory(directory)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("refresh.lock"))?;
    lock.try_lock()?;
    progress("Checking production logs…".into());
    let inventory = describe(remote)?;
    let mut bytes = 0;
    let history = refresh_cache(
        directory,
        remote,
        inventory,
        |segment, file, from, to| {
            transfer(remote, segment, file, from, to, &progress)?;
            bytes += to - from;
            Ok(())
        },
        &progress,
    )?;
    Ok((history, bytes))
}

fn refresh_cache(
    directory: &Path, remote: &str, inventory: Inventory,
    mut fetch: impl FnMut(&Segment, &mut File, u64, u64) -> Result<()>, progress: impl Fn(String),
) -> Result<History> {
    let manifest_path = directory.join("manifest.json");
    let mut manifest = read_manifest(&manifest_path, remote)?;
    let realigned = inventory.segments.iter().any(|segment| {
        manifest
            .segments
            .get(&segment.number)
            .is_some_and(|cached| {
                let prior = &cached.source;
                prior.identity != segment.identity
                    || prior.length > segment.length
                    || (prior.length == segment.length && prior.stamp != segment.stamp)
            })
    }) || manifest
        .segments
        .last_key_value()
        .is_some_and(|(&number, _)| number > inventory.segments.last().unwrap().number);
    if realigned || manifest.generation.is_empty() {
        manifest.segments.clear();
        let generation = tempfile::Builder::new()
            .prefix("ranges-")
            .tempdir_in(directory)?
            .keep();
        manifest.generation = generation
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
    }
    let range_directory = directory.join(&manifest.generation);

    let mut segments: BTreeMap<_, _> = manifest
        .segments
        .values()
        .map(|cached| (cached.source.number, (cached.source.clone(), false)))
        .collect();
    segments.extend(
        inventory
            .segments
            .into_iter()
            .map(|segment| (segment.number, (segment, true))),
    );
    let mut previous: Option<Tail> = None;
    for (segment, on_server) in segments.values() {
        let existing = manifest.segments.get(&segment.number);
        let contiguous = previous
            .as_ref()
            .is_some_and(|tail| tail.closed && tail.last == Some(segment.number));
        let mut skip = offset_for(segment, contiguous, existing);
        let path = segment_path(&range_directory, segment);
        if !*on_server && (!path.is_file() || (skip != 0 && !contiguous)) {
            manifest.segments.remove(&segment.number);
            continue;
        }
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        if skip != 0 && !contiguous {
            let mut snapshot = tempfile::tempfile_in(directory)?;
            fetch(segment, &mut snapshot, 0, skip)?;
            snapshot.rewind()?;
            file.rewind()?;
            io::copy(&mut snapshot, &mut file)?;
            file.sync_all()?;
            skip = 0;
        }
        if file.metadata()?.len() < skip {
            file.set_len(skip)?;
        }
        if file.metadata()?.len() > segment.length {
            file.set_len(skip)?;
        }
        let known = existing.map(|cached| cached.tail.clone()).filter(|tail| {
            tail.offset >= skip
                && tail.offset <= file.metadata().map_or(0, |metadata| metadata.len())
        });
        let mut tail = if let Some(known) = known {
            scan_from(&path, segment.number, known, |_, _, _| Ok(()))?
        } else {
            scan(&path, segment.number, skip, |_, _, _| Ok(()))?
        };
        let unchanged_partial = tail.partial
            && file.metadata()?.len() == segment.length
            && existing.is_some_and(|cached| cached.source == *segment);
        if tail.offset < segment.length && *on_server && !unchanged_partial {
            file.set_len(tail.offset)?;
            file.seek(SeekFrom::Start(tail.offset))?;
            if let Err(error) = fetch(segment, &mut file, tail.offset, segment.length) {
                manifest
                    .segments
                    .insert(segment.number, CachedSegment { source: segment.clone(), skip, tail });
                save_manifest(directory, &manifest_path, &manifest)?;
                return Err(error);
            }
            tail = scan_from(&path, segment.number, tail, |_, _, _| Ok(()))?;
        }
        previous = Some(tail.clone());
        manifest
            .segments
            .insert(segment.number, CachedSegment { source: segment.clone(), skip, tail });
    }
    save_manifest(directory, &manifest_path, &manifest)?;
    let signature = Sha256::digest(serde_json::to_vec(&manifest)?).into();
    if let Ok(Some(mut history)) = super::read_cache(directory) {
        if history.import_signature == Some(signature) {
            if !history.partial_tail {
                history.anchor(inventory.observed, history.latest);
            }
            return super::write_cache(directory, history);
        }
    }
    progress("Replaying transactions…".into());
    let mut history = replay(
        manifest
            .segments
            .into_values()
            .map(|cached| {
                (cached.source.number, segment_path(&range_directory, &cached.source), cached.skip)
            })
            .collect(),
        Some(inventory.observed),
    )?;
    history.import_signature = Some(signature);
    super::write_cache(directory, history)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use db_rs::View;
    use db_rs::config::Config;
    use lb::model::account::Account as CoreAccount;
    use lb::model::file_like::FileLike;
    use lb::model::file_metadata::Owner;
    use lb::model::meta::Meta;
    use lb::model::server_meta::IntoServerMeta;
    use lockbook_server_lib::guard::ServerTx;
    use lockbook_server_lib::schema::{Account, ServerDb};

    fn fixture(path: &Path) -> (ServerDb, Owner) {
        let mut db = ServerDb::init(&Config::default().log_location(path.to_path_buf())).unwrap();
        let account = CoreAccount::new("metrics".into(), "http://localhost".into());
        let owner = Owner(account.public_key());
        let root = Meta::create_root(&account)
            .unwrap()
            .sign_with(&account)
            .unwrap();
        let created = root.timestamped_value.timestamp;
        let root = root.add_time(created as u64);
        let mut tx = ServerTx::begin(&mut db).unwrap();
        tx.accounts
            .insert(owner, Account { username: account.username, billing_info: Default::default() })
            .unwrap();
        tx.metas.insert(*root.id(), root.clone()).unwrap();
        tx.owned_files.insert(owner, *root.id()).unwrap();
        tx.shared_files.create_key(owner).unwrap();
        tx.file_children.create_key(*root.id()).unwrap();
        tx.end().unwrap();
        (db, owner)
    }

    fn activity(db: &mut ServerDb, owner: Owner, time: u64) {
        let mut tx = ServerTx::begin(db).unwrap();
        tx.last_seen.insert(owner, time).unwrap();
        tx.end().unwrap();
    }

    fn inventory(path: &Path) -> Inventory {
        let output = Command::new("python3")
            .args(["-c", REMOTE])
            .arg(path)
            .arg("inventory")
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", String::from_utf8_lossy(&output.stderr));
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn import(
        original: &Path, cache: &Path, received: &mut Vec<(u64, u64, u64)>, interrupt: Option<u64>,
    ) -> Result<History> {
        refresh_cache(
            cache,
            "/fixture/ServerV6",
            inventory(original),
            |segment, destination, start, end| {
                received.push((segment.number, start, end));
                let mut reader = File::open(original.join(format!("db.{}.log", segment.number)))?;
                reader.seek(SeekFrom::Start(start))?;
                let stop = interrupt.unwrap_or(end).min(end);
                io::copy(&mut io::Read::take(reader, stop - start), destination)?;
                if stop != end {
                    return Err("simulated interrupted transfer".into());
                }
                Ok(())
            },
            |_| {},
        )
    }

    #[test]
    fn transfers_only_the_baseline_and_new_frames_after_rotation() {
        let source = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let (mut db, owner) = fixture(source.path());
        let created = db.schema.metas.iter().next().unwrap().1.version;
        activity(&mut db, owner, created + 10_000);
        db.snapshot().unwrap();
        activity(&mut db, owner, created + 20_000);
        db.snapshot().unwrap();
        let mut ranges = Vec::new();
        let history = import(source.path(), cache.path(), &mut ranges, None).unwrap();
        let local_history = local(source.path()).unwrap();
        let catalog = inventory(source.path());
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].0, 0);
        assert_eq!(ranges[1], (2, catalog.segments[1].first_end, catalog.segments[1].length));
        assert_eq!(history.latest, 3);
        assert_eq!(history.accounts[0].activity.len(), 2);
        assert_eq!(history.latest, local_history.latest);
        assert_eq!(history.accounts[0].activity, local_history.accounts[0].activity);
        assert_eq!(history.checkpoint(2).unwrap().users, db.schema.accounts.len());

        ranges.clear();
        import(source.path(), cache.path(), &mut ranges, None).unwrap();
        assert!(ranges.is_empty());
        activity(&mut db, owner, created + 30_000);
        let updated = import(source.path(), cache.path(), &mut ranges, None).unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].0, 3);
        assert_eq!(ranges[0].1, catalog.segments[2].first_end);
        assert_eq!(updated.latest, 4);
    }

    #[test]
    fn resumes_from_the_last_complete_frame() {
        let source = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let (mut db, owner) = fixture(source.path());
        let first_end = fs::metadata(source.path().join("db.0.log")).unwrap().len();
        let stamp = db.schema.metas.iter().next().unwrap().1.version + 10_000;
        activity(&mut db, owner, stamp);
        let mut ranges = Vec::new();
        assert!(import(source.path(), cache.path(), &mut ranges, Some(first_end + 4)).is_err());
        assert!(super::super::read_cache(cache.path()).unwrap().is_none());
        let history = import(source.path(), cache.path(), &mut ranges, None).unwrap();
        assert_eq!(ranges[1].1, first_end);
        assert_eq!(history.latest, 2);
    }

    #[test]
    fn missing_segment_starts_again_from_a_snapshot() {
        let source = tempfile::tempdir().unwrap();
        let (mut db, owner) = fixture(source.path());
        let stamp = db.schema.metas.iter().next().unwrap().1.version;
        db.snapshot().unwrap();
        activity(&mut db, owner, stamp + 10_000);
        db.snapshot().unwrap();
        activity(&mut db, owner, stamp + 20_000);
        fs::remove_file(source.path().join("db.1.log")).unwrap();
        let history = local(source.path()).unwrap();
        assert_eq!(history.gaps, [(1, 2)]);
        assert_eq!(history.latest, 3);
        assert_eq!(history.accounts[0].activity.len(), 1);
        assert_eq!(history.checkpoint(3).unwrap().users, 1);
    }

    #[test]
    fn a_restored_source_only_replaces_the_view_after_replay() {
        let source = tempfile::tempdir().unwrap();
        let replacement = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let (mut db, owner) = fixture(source.path());
        let stamp = db.schema.metas.iter().next().unwrap().1.version;
        activity(&mut db, owner, stamp + 10_000);
        let mut ranges = Vec::new();
        import(source.path(), cache.path(), &mut ranges, None).unwrap();
        let (_db, _) = fixture(replacement.path());
        assert!(import(replacement.path(), cache.path(), &mut ranges, Some(4)).is_err());
        assert_eq!(
            super::super::read_cache(cache.path())
                .unwrap()
                .unwrap()
                .latest,
            2
        );
        assert_eq!(
            import(replacement.path(), cache.path(), &mut ranges, None)
                .unwrap()
                .latest,
            1
        );
    }

    #[test]
    fn remote_helper_streams_only_the_inventoried_range() {
        let source = tempfile::tempdir().unwrap();
        let (mut db, owner) = fixture(source.path());
        let stamp = db.schema.metas.iter().next().unwrap().1.version;
        let segment = inventory(source.path()).segments.pop().unwrap();
        let bytes = fs::read(source.path().join("db.0.log")).unwrap();
        activity(&mut db, owner, stamp + 10_000);
        let output = Command::new("python3")
            .args(["-c", REMOTE])
            .arg(source.path())
            .args([
                "read",
                "0",
                &segment.identity,
                &segment.stamp.to_string(),
                &segment.length.to_string(),
                "8",
                &segment.length.to_string(),
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        let mut range = Vec::new();
        GzDecoder::new(&output.stdout[..])
            .read_to_end(&mut range)
            .unwrap();
        assert_eq!(range, bytes[8..]);
    }
}
