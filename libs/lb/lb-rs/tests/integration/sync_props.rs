//! Byte-stream properties of sync: devices on accounts take random steps against one in-memory
//! server, and each property holds at the widest tier whose steps it survives, from every start.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use lb_rs::io::server::Server;
use lb_rs::model::account::Account;
use lb_rs::model::api::{AdminValidateServer, AdminValidateServerRequest};
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::{File, ShareMode};
use lb_rs::model::file_metadata::FileType;
use test_utils::byte_source::ByteSource;
use test_utils::gate::{Gate, GatedDocuments, Point};
use test_utils::lockbook_server_lib::ServerState;
use test_utils::lockbook_server_lib::billing::Nop;
use test_utils::lockbook_server_lib::in_process::InMemDocuments;
use test_utils::{prop, test_config};
use tokio::sync::{Notify, oneshot};
use uuid::Uuid;

type TestServer = ServerState<Nop, Nop, Nop, GatedDocuments<InMemDocuments>>;
type Lb = lb_rs::Lb<TestServer>;

mod starts;

// ---------------------------------------------------------------------------------------------
// property table
// ---------------------------------------------------------------------------------------------

// Each tier is the highest where every seed passes in a local run; CI's run stops at its time
// budget, so it never sets one.

#[test]
fn syncing_alone_changes_nothing() {
    run(OVERLAP, 2000, syncing_alone_changes_nothing_check);
}

#[test]
fn disjoint_changes_equal_sequential() {
    run(WRITE_SHARES, 2000, disjoint_changes_equal_sequential_check);
}

#[test]
fn a_new_device_sees_what_was_synced() {
    run(WRITE_SHARES, 2000, a_new_device_sees_what_was_synced_check);
}

#[test]
fn a_refused_step_changes_nothing() {
    run(WRITE_SHARES, 1000, a_refused_step_changes_nothing_check);
}

#[test]
fn a_second_sync_changes_nothing() {
    run(WRITE_SHARES, 2000, a_second_sync_changes_nothing_check);
}

#[test]
fn devices_converge() {
    run(WRITE_SHARES, 2000, devices_converge_check);
}

#[test]
fn accounts_agree_on_what_they_share() {
    run(WRITE_SHARES, 2000, accounts_agree_on_what_they_share_check);
}

#[test]
fn the_server_stays_valid() {
    run(WRITE_SHARES, 2000, the_server_stays_valid_check);
}

// ---------------------------------------------------------------------------------------------
// tiers
// ---------------------------------------------------------------------------------------------

/// The steps a case may take and the starts it takes them from. A property runs at the widest
/// tier whose cases it survives.
#[derive(Clone, Copy, Debug)]
struct Tier {
    accounts: usize,
    devices: usize,
    /// Whether a step may run while a sync waits on the server.
    overlap: bool,
    starts: &'static [Start],
}

/// Several accounts with two devices each, each device syncing when it likes, sharing with one
/// another.
const SHARES: Tier =
    Tier { accounts: 3, devices: 2, overlap: false, starts: &[Start::Empty, Start::Shared] };

/// [`SHARES`] with steps taken while a sync waits on the server.
const OVERLAP: Tier = Tier { overlap: true, ..SHARES };

/// [`OVERLAP`] also from write shares several accounts reorganize: folders nested in one, and a
/// trip folder friends write to.
const WRITE_SHARES: Tier =
    Tier { starts: &[Start::Empty, Start::Shared, Start::WriteShared, Start::Vacation], ..OVERLAP };

impl Tier {
    fn all_devices(self) -> usize {
        self.accounts * self.devices
    }
}

/// What every device holds before the steps.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Start {
    Empty,
    /// Each account has a folder `a.md` holding a document and a folder `c`, shared with the
    /// next account, which links it as `b.md`. Every device has synced all of it, and steps'
    /// names collide with it.
    Shared,
    /// [`Start::Shared`] with `c` holding `c/c/`, and shared for writing.
    WriteShared,
    /// Four friends with two devices each write to a shared trip folder; see [`starts`].
    Vacation,
}

/// How often each kind of step comes up. Mixes sample a tier's cases differently and never
/// widen it.
#[derive(Clone, Copy, Debug)]
enum Mix {
    Even,
    /// Writes, and steps during a sync, four times as often.
    Writes,
}

const MIXES: [Mix; 2] = [Mix::Even, Mix::Writes];

impl Mix {
    fn writes(self) -> u32 {
        match self {
            Mix::Even => 1,
            Mix::Writes => 4,
        }
    }
}

/// One way of drawing cases: a tier's steps, from one of its starts, in one mix.
#[derive(Clone, Copy, Debug)]
struct Case {
    tier: Tier,
    start: Start,
    mix: Mix,
}

// ---------------------------------------------------------------------------------------------
// steps
// ---------------------------------------------------------------------------------------------

/// Ordered boring to exotic: an exhausted buffer picks the first. Indexes pick among the
/// device's files sorted by path, so a buffer names the same files on every run.
#[derive(Clone, Debug)]
enum Step {
    Sync {
        device: usize,
    },
    SyncAll {
        device: usize,
    },
    CreateDocument {
        device: usize,
        parent: usize,
        name: usize,
    },
    CreateFolder {
        device: usize,
        parent: usize,
        name: usize,
    },
    Write {
        device: usize,
        file: usize,
        content: usize,
    },
    Rename {
        device: usize,
        file: usize,
        name: usize,
    },
    Move {
        device: usize,
        file: usize,
        parent: usize,
    },
    Delete {
        device: usize,
        file: usize,
    },
    Share {
        device: usize,
        file: usize,
        to: usize,
        write: bool,
    },
    Accept {
        device: usize,
        pending: usize,
        name: usize,
    },
    Reject {
        device: usize,
        pending: usize,
    },
    Unshare {
        device: usize,
        file: usize,
        from: usize,
    },
    /// `device` syncs, and when the server reaches its `at`th document read or write of that
    /// sync, `then` runs before the server goes on.
    SyncDuring {
        device: usize,
        at: usize,
        then: Box<Step>,
    },
}

// few names, so devices collide on paths; mostly text, as people's files are
const NAMES: [&str; 5] = ["a.md", "b.md", "c", "d.json", "e.svg"];
const NAME_WEIGHTS: [u32; 5] = [27, 27, 26, 10, 10];

const TEXTS: [&str; 3] = ["one", "two\nlines", ""];
// each kind shares a base with its siblings, so concurrent writes merge; "one" isn't valid
const JSONS: [&str; 5] = [
    r#"{"a":1,"b":"x"}"#,
    r#"{"a":2,"b":"x"}"#,
    r#"{"a":1,"tags":["p","q"],"tasks":{"t1":{"done":false},"t2":{"done":false}}}"#,
    r#"{"a":1,"tags":["q","r"],"tasks":{"t1":{"done":true}}}"#,
    "one",
];
// strokes 1 and 2, then: add 3, add 4, move 1, delete 1
const DRAWINGS: [&str; 6] = [
    r#"<svg xmlns="http://www.w3.org/2000/svg"><path id="01010101-0101-0101-0101-010101010101" d="M 1 0 L 1 10 " stroke="rgb(0,0,0)" stroke-width="1" fill="none"/><path id="02020202-0202-0202-0202-020202020202" d="M 2 0 L 2 10 " stroke="rgb(0,0,0)" stroke-width="1" fill="none"/></svg>"#,
    r#"<svg xmlns="http://www.w3.org/2000/svg"><path id="01010101-0101-0101-0101-010101010101" d="M 1 0 L 1 10 " stroke="rgb(0,0,0)" stroke-width="1" fill="none"/><path id="02020202-0202-0202-0202-020202020202" d="M 2 0 L 2 10 " stroke="rgb(0,0,0)" stroke-width="1" fill="none"/><path id="03030303-0303-0303-0303-030303030303" d="M 3 0 L 3 10 " stroke="rgb(0,0,0)" stroke-width="1" fill="none"/></svg>"#,
    r#"<svg xmlns="http://www.w3.org/2000/svg"><path id="01010101-0101-0101-0101-010101010101" d="M 1 0 L 1 10 " stroke="rgb(0,0,0)" stroke-width="1" fill="none"/><path id="02020202-0202-0202-0202-020202020202" d="M 2 0 L 2 10 " stroke="rgb(0,0,0)" stroke-width="1" fill="none"/><path id="04040404-0404-0404-0404-040404040404" d="M 4 0 L 4 10 " stroke="rgb(0,0,0)" stroke-width="1" fill="none"/></svg>"#,
    r#"<svg xmlns="http://www.w3.org/2000/svg"><path id="01010101-0101-0101-0101-010101010101" d="M 5 0 L 5 10 " stroke="rgb(0,0,0)" stroke-width="1" fill="none"/><path id="02020202-0202-0202-0202-020202020202" d="M 2 0 L 2 10 " stroke="rgb(0,0,0)" stroke-width="1" fill="none"/></svg>"#,
    r#"<svg xmlns="http://www.w3.org/2000/svg"><path id="02020202-0202-0202-0202-020202020202" d="M 2 0 L 2 10 " stroke="rgb(0,0,0)" stroke-width="1" fill="none"/></svg>"#,
    "one",
];

/// The contents a write may give the document named `name`.
fn contents(name: &str) -> &'static [&'static str] {
    if name.ends_with(".json") {
        &JSONS
    } else if name.ends_with(".svg") {
        &DRAWINGS
    } else {
        &TEXTS
    }
}

fn steps(case: Case, bytes: &[u8]) -> Vec<Step> {
    let tier = case.tier;
    let mut src = ByteSource::new(bytes);
    let count = 1 + src.draw(48);
    let during = if tier.overlap { case.mix.writes() } else { 0 };
    let mut device = 0;
    (0..count)
        .map(|_| {
            // people work on one device for a while
            if src.bias(&[4, 1]) == 1 {
                device = src.draw(tier.all_devices());
            }
            if src.bias(&[10, during]) == 1 {
                let at = src.draw(4);
                let on = src.draw(tier.all_devices());
                let then = match step(case, &mut src, on) {
                    // a sync waits for the one in flight on its device
                    Step::SyncAll { device } => Step::Sync { device },
                    then => then,
                };
                return Step::SyncDuring { device, at, then: Box::new(then) };
            }
            step(case, &mut src, device)
        })
        .collect()
}

/// One step on `device` that runs alone.
fn step(case: Case, src: &mut ByteSource, device: usize) -> Step {
    let tier = case.tier;
    let shares = if tier.accounts > 1 { 2 } else { 0 };
    let writes = 3 * case.mix.writes();
    match src.bias(&[3, 1, 3, 2, writes, 2, 2, 1, shares, shares, shares, shares]) {
        0 => Step::Sync { device },
        1 => Step::SyncAll { device },
        2 => Step::CreateDocument { device, parent: src.draw(8), name: src.bias(&NAME_WEIGHTS) },
        3 => Step::CreateFolder { device, parent: src.draw(8), name: src.bias(&NAME_WEIGHTS) },
        4 => Step::Write { device, file: src.draw(8), content: src.draw(8) },
        5 => Step::Rename { device, file: src.draw(8), name: src.bias(&NAME_WEIGHTS) },
        6 => Step::Move { device, file: src.draw(8), parent: src.draw(8) },
        7 => Step::Delete { device, file: src.draw(8) },
        8 => Step::Share {
            device,
            file: src.draw(8),
            to: src.draw(tier.accounts),
            write: src.draw(2) == 1,
        },
        9 => Step::Accept { device, pending: src.draw(8), name: src.bias(&NAME_WEIGHTS) },
        10 => Step::Reject { device, pending: src.draw(8) },
        _ => Step::Unshare { device, file: src.draw(8), from: src.draw(4) },
    }
}

impl Step {
    fn device(&self) -> usize {
        match *self {
            Step::Sync { device }
            | Step::SyncAll { device }
            | Step::CreateDocument { device, .. }
            | Step::CreateFolder { device, .. }
            | Step::Write { device, .. }
            | Step::Rename { device, .. }
            | Step::Move { device, .. }
            | Step::Delete { device, .. }
            | Step::Share { device, .. }
            | Step::Accept { device, .. }
            | Step::Reject { device, .. }
            | Step::Unshare { device, .. }
            | Step::SyncDuring { device, .. } => device,
        }
    }

    /// This step, with any step it runs during, taken on `on`.
    fn confined(&self, on: usize) -> Step {
        let mut step = self.clone();
        match &mut step {
            Step::SyncDuring { device, then, .. } => {
                *device = on;
                **then = then.confined(on);
            }
            Step::Sync { device }
            | Step::SyncAll { device }
            | Step::CreateDocument { device, .. }
            | Step::CreateFolder { device, .. }
            | Step::Write { device, .. }
            | Step::Rename { device, .. }
            | Step::Move { device, .. }
            | Step::Delete { device, .. }
            | Step::Share { device, .. }
            | Step::Accept { device, .. }
            | Step::Reject { device, .. }
            | Step::Unshare { device, .. } => *device = on,
        }
        step
    }
}

/// Which files a step may touch: any, or only those named for one device.
#[derive(Clone, Copy)]
enum Reach {
    Shared,
    Own(usize),
}

impl Reach {
    fn name(self, index: usize) -> String {
        match self {
            Reach::Shared => NAMES[index].to_string(),
            Reach::Own(device) => format!("{device}-{}", NAMES[index]),
        }
    }

    fn reaches(self, name: &str) -> bool {
        match self {
            Reach::Shared => true,
            Reach::Own(device) => name.starts_with(&format!("{device}-")),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// world
// ---------------------------------------------------------------------------------------------

/// Accounts with their devices, an admin, and the server they share. Devices are numbered
/// account by account.
struct World {
    server: TestServer,
    pause: Pause,
    admin: Account,
    names: Vec<String>,
    devices: Vec<Lb>,
    per_account: usize,
}

impl World {
    /// A copy of `case`'s start, whose devices draw new ids from `bytes`.
    async fn new(case: Case, bytes: &[u8]) -> Self {
        starts::built(case.tier, case.start).copy(seed(bytes)).await
    }

    /// `tier`'s accounts with nothing in them, every id drawn from `seed`.
    async fn empty(tier: Tier, seed: u64) -> Self {
        let pause = Pause::default();
        let mut server = TestServer::with_documents(GatedDocuments::new(
            InMemDocuments::default(),
            pause.clone(),
        ));
        let admin_name = name(seed, 0);
        server.config.admin.admins.insert(admin_name.clone());
        let admin = device(&server, seed, 0).await;
        let admin = admin.create_account(&admin_name, "", false).await.unwrap();

        let mut names = Vec::new();
        let mut devices = Vec::new();
        for account in 0..tier.accounts {
            let name = name(seed, account + 1);
            let first = device(&server, seed, devices.len() + 1).await;
            first.create_account(&name, "", false).await.unwrap();
            let key = first.export_account_private_key().unwrap();
            devices.push(first);
            for _ in 1..tier.devices {
                let next = device(&server, seed, devices.len() + 1).await;
                next.import_account(&key, None).await.unwrap();
                next.sync().await.unwrap();
                devices.push(next);
            }
            names.push(name);
        }
        Self { server, pause, admin, names, devices, per_account: tier.devices }
    }

    /// A world holding what this one holds, sharing nothing with it, whose devices draw new ids
    /// from `seed`.
    async fn copy(&self, seed: u64) -> Self {
        let pause = Pause::default();
        let documents =
            GatedDocuments::new(self.server.document_service.docs.copy(), pause.clone());
        let server = self.server.copy_with(documents).await;
        let mut devices = Vec::new();
        for (n, device) in self.devices.iter().enumerate() {
            let config = config(seed, n + 1);
            devices.push(device.copy_in_memory(config, server.clone()).await.unwrap());
        }
        Self {
            server,
            pause,
            admin: self.admin.clone(),
            names: self.names.clone(),
            devices,
            per_account: self.per_account,
        }
    }

    /// The [`Start::Shared`] files, with every device synced past them, `depth` folders deep
    /// and shared as `mode`.
    async fn start_shared(&self, depth: usize, mode: ShareMode) {
        let accounts = self.names.len();
        for account in 0..accounts {
            let device = &self.account_devices(account)[0];
            let root = device.root().await.unwrap().id;
            let folder = device
                .create_file(NAMES[0], &root, FileType::Folder)
                .await
                .unwrap();
            device
                .create_file(NAMES[1], &folder.id, FileType::Document)
                .await
                .unwrap();
            let shared = device
                .create_file(NAMES[2], &root, FileType::Folder)
                .await
                .unwrap();
            let mut inner = shared.id;
            for _ in 1..depth {
                inner = device
                    .create_file(NAMES[2], &inner, FileType::Folder)
                    .await
                    .unwrap()
                    .id;
            }
            let next = &self.names[(account + 1) % accounts];
            device.share_file(shared.id, next, mode).await.unwrap();
            device.sync().await.unwrap();
        }
        for account in 0..accounts {
            let device = &self.account_devices(account)[0];
            device.sync().await.unwrap();
            for pending in device.get_pending_shares().await.unwrap() {
                device
                    .create_link_at_path(NAMES[1], pending.id)
                    .await
                    .unwrap();
            }
        }
        self.settle().await.unwrap();
    }

    fn account_devices(&self, account: usize) -> &[Lb] {
        &self.devices[account * self.per_account..(account + 1) * self.per_account]
    }

    /// Takes `step` on device `on`, returning whether the device accepted it.
    async fn take(&self, step: &Step, on: usize, reach: Reach) -> bool {
        let device = &self.devices[on];
        match *step {
            Step::SyncDuring { at, ref then, .. } => {
                // a second sync on a device waits for its first
                if matches!(**then, Step::Sync { device } if device == on) {
                    return device.sync().await.is_ok();
                }
                let then_reach = match reach {
                    Reach::Shared => Reach::Shared,
                    Reach::Own(_) => Reach::Own(then.device()),
                };
                let parked = self.pause.arm(at);
                let syncing = async {
                    let synced = device.sync().await.is_ok();
                    self.pause.disarm();
                    synced
                };
                let meanwhile = async {
                    if parked.await.is_err() {
                        return Box::pin(self.take(then, then.device(), then_reach)).await;
                    }
                    PAUSES.set(PAUSES.get() + 1);
                    // the server stamps versions in milliseconds
                    tokio::time::sleep(Duration::from_millis(2)).await;
                    let taken = Box::pin(self.take(then, then.device(), then_reach)).await;
                    self.pause.release();
                    taken
                };
                let both = async { tokio::join!(syncing, meanwhile) };
                let (synced, taken) = tokio::time::timeout(Duration::from_secs(10), both)
                    .await
                    .expect("a step taken during a sync finishes");
                synced && taken
            }
            Step::Sync { .. } => device.sync().await.is_ok(),
            Step::SyncAll { .. } => {
                let mut all = true;
                for device in &self.devices {
                    all &= device.sync().await.is_ok();
                }
                all
            }
            Step::CreateDocument { parent, name, .. } | Step::CreateFolder { parent, name, .. } => {
                let file_type = match step {
                    Step::CreateFolder { .. } => FileType::Folder,
                    _ => FileType::Document,
                };
                let parent = pick(device, reach, parent, Kind::Folder).await.unwrap();
                device
                    .create_file(&reach.name(name), &parent, file_type)
                    .await
                    .is_ok()
            }
            Step::Write { file, content, .. } => {
                let Some(file) = pick(device, reach, file, Kind::Document).await else {
                    return false;
                };
                let Ok(name) = device.get_file_by_id(file).await.map(|f| f.name) else {
                    return false;
                };
                let contents = contents(&name);
                device
                    .write_document(file, contents[content % contents.len()].as_bytes())
                    .await
                    .is_ok()
            }
            Step::Rename { file, name, .. } => {
                let Some(file) = pick(device, reach, file, Kind::Any).await else {
                    return false;
                };
                device.rename_file(&file, &reach.name(name)).await.is_ok()
            }
            Step::Move { file, parent, .. } => {
                let Some(file) = pick(device, reach, file, Kind::Any).await else {
                    return false;
                };
                let parent = pick(device, reach, parent, Kind::Folder).await.unwrap();
                device.move_file(&file, &parent).await.is_ok()
            }
            Step::Delete { file, .. } => {
                let Some(file) = pick(device, reach, file, Kind::Any).await else {
                    return false;
                };
                device.delete(&file).await.is_ok()
            }
            Step::Share { file, to, write, .. } => {
                let Some(file) = pick(device, reach, file, Kind::Any).await else {
                    return false;
                };
                let mode = if write { ShareMode::Write } else { ShareMode::Read };
                device
                    .share_file(file, &self.names[to % self.names.len()], mode)
                    .await
                    .is_ok()
            }
            Step::Accept { pending, name, .. } => {
                let Some((device, target)) = self.pending_from(on, pending).await else {
                    return false;
                };
                device
                    .create_link_at_path(&reach.name(name), target)
                    .await
                    .is_ok()
            }
            Step::Reject { pending, .. } => {
                let Some((device, target)) = self.pending_from(on, pending).await else {
                    return false;
                };
                device.reject_share(&target).await.is_ok()
            }
            Step::Unshare { file, from, .. } => {
                let Some(file) = pick(device, reach, file, Kind::Shared).await else {
                    return false;
                };
                let shares = device.get_file_by_id(file).await.unwrap().shares;
                let sharee = &shares[from % shares.len()].shared_with;
                device.unshare_file(file, sharee).await.is_ok()
            }
        }
    }

    /// A share waiting for the first device, counting on from `on`, that has one.
    async fn pending_from(&self, on: usize, index: usize) -> Option<(&Lb, Uuid)> {
        for offset in 0..self.devices.len() {
            let device = &self.devices[(on + offset) % self.devices.len()];
            if let Some(target) = pick_pending(device, index).await {
                return Some((device, target));
            }
        }
        None
    }

    /// Takes every step on the device it names.
    async fn take_all(&self, steps: &[Step]) {
        for step in steps {
            self.take(step, step.device(), Reach::Shared).await;
        }
    }

    /// Syncs every device until each has seen every other's changes, shares included.
    async fn settle(&self) -> Result<(), String> {
        for _ in 0..3 {
            for (i, device) in self.devices.iter().enumerate() {
                device
                    .sync()
                    .await
                    .map_err(|err| format!("device {i} failed to sync: {err:?}"))?;
            }
        }
        Ok(())
    }

    async fn validate_server(&self) -> Result<(), String> {
        let report =
            Server::admin_validate_server(&self.server, &self.admin, AdminValidateServerRequest {})
                .await
                .map_err(|err| format!("server validation failed to run: {err:?}"))?;
        if report != AdminValidateServer::default() {
            return Err(format!("server is invalid: {report:?}"));
        }
        Ok(())
    }
}

type Armed = (usize, oneshot::Sender<()>);

/// Parks the server at a document read or write a step chose, until the step lets it go on.
#[derive(Clone, Default)]
struct Pause {
    /// How many operations to let by, and who hears when the next one parks.
    armed: Arc<Mutex<Option<Armed>>>,
    release: Arc<Notify>,
}

impl Pause {
    /// The receiver hears when the store's `at`th operation from now parks, or errs if
    /// [`Self::disarm`] comes first.
    fn arm(&self, at: usize) -> oneshot::Receiver<()> {
        let (entered, parked) = oneshot::channel();
        *self.armed.lock().unwrap() = Some((at, entered));
        parked
    }

    fn disarm(&self) {
        self.armed.lock().unwrap().take();
    }

    fn release(&self) {
        self.release.notify_one();
    }
}

#[async_trait]
impl Gate for Pause {
    async fn pass(&self, _: Point) {
        let entered = {
            let mut armed = self.armed.lock().unwrap();
            match armed.take() {
                Some((0, entered)) => Some(entered),
                Some((at, entered)) => {
                    *armed = Some((at - 1, entered));
                    None
                }
                None => None,
            }
        };
        if let Some(entered) = entered {
            let _ = entered.send(());
            self.release.notified().await;
        }
    }
}

/// The seed a case's devices draw ids from: the same bytes give the same ids on every run.
fn seed(bytes: &[u8]) -> u64 {
    // FNV-1a, since std's hashers are seeded per process
    bytes
        .iter()
        .fold(0xcbf29ce484222325, |hash, &byte| (hash ^ byte as u64).wrapping_mul(0x100000001b3))
}

/// The config of the `n`th device a world seeded with `seed` makes.
fn config(seed: u64, n: usize) -> lb_rs::model::core_config::Config {
    // splitmix64, so neighboring devices draw unrelated ids
    let mut z = seed.wrapping_add((n as u64 + 1).wrapping_mul(0x9e3779b97f4a7c15));
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    lb_rs::model::core_config::Config { seed: Some(z ^ (z >> 31)), ..test_config() }
}

/// The `n`th username a world seeded with `seed` gives out.
fn name(seed: u64, n: usize) -> String {
    format!("u{:x}", config(seed, n).seed.unwrap())
}

/// The `n`th device a world seeded with `seed` makes.
async fn device(server: &TestServer, seed: u64, n: usize) -> Lb {
    Lb::in_memory(config(seed, n), server.clone())
        .await
        .unwrap()
}

/// What a device shows: every file's type, shares and content, keyed by path.
async fn tree(device: &Lb) -> BTreeMap<String, String> {
    let mut tree = BTreeMap::new();
    for file in device.list_metadatas().await.unwrap() {
        let mut shown = format!("{:?}{}", file.file_type, shares(&file));
        if file.is_document() {
            write!(shown, " {}", read(device, file.id).await).unwrap();
        }
        tree.insert(device.get_path_by_id(file.id).await.unwrap(), shown);
    }
    tree
}

/// Each document a device lists, by id, with its shares and content.
async fn documents(device: &Lb) -> BTreeMap<Uuid, String> {
    let mut documents = BTreeMap::new();
    for file in device.list_metadatas().await.unwrap() {
        if file.is_document() {
            documents.insert(file.id, format!("{}{}", shares(&file), read(device, file.id).await));
        }
    }
    documents
}

/// A listed document's content. One the server no longer serves this device reads as gone until
/// the next sync.
async fn read(device: &Lb, id: Uuid) -> String {
    match device.read_document(id, false).await {
        Ok(content) => format!("{:?}", String::from_utf8_lossy(&content)),
        Err(err)
            if matches!(
                err.kind,
                LbErrKind::FileNonexistent | LbErrKind::InsufficientPermission
            ) =>
        {
            "gone".into()
        }
        Err(err) => panic!("a listed document failed to read: {:?}", err.kind),
    }
}

/// A file's shares as `by>with:mode`, sorted.
fn shares(file: &File) -> String {
    let mut shares: Vec<_> = file
        .shares
        .iter()
        .map(|share| format!(" {}>{}:{:?}", share.shared_by, share.shared_with, share.mode))
        .collect();
    shares.sort();
    shares.concat()
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Any,
    Shared,
    Document,
    Folder,
}

/// The `index`th file of `kind` within `reach`, by path. Folders include the root.
async fn pick(device: &Lb, reach: Reach, index: usize, kind: Kind) -> Option<Uuid> {
    let mut candidates = Vec::new();
    for file in device.list_metadatas().await.unwrap() {
        let is_root = file.parent == file.id;
        let fits = match kind {
            Kind::Any => !is_root && reach.reaches(&file.name),
            Kind::Shared => !file.shares.is_empty() && reach.reaches(&file.name),
            Kind::Document => file.is_document() && reach.reaches(&file.name),
            Kind::Folder => file.is_folder() && (is_root || reach.reaches(&file.name)),
        };
        if fits {
            candidates.push((device.get_path_by_id(file.id).await.unwrap(), file.id));
        }
    }
    candidates.sort();
    candidates
        .get(index % candidates.len().max(1))
        .map(|(_, id)| *id)
}

/// The `index`th share waiting for this device's account, by name.
async fn pick_pending(device: &Lb, index: usize) -> Option<Uuid> {
    let mut pending: Vec<_> = device
        .get_pending_shares()
        .await
        .unwrap()
        .into_iter()
        .map(|f| (f.name, f.id))
        .collect();
    pending.sort();
    pending.get(index % pending.len().max(1)).map(|(_, id)| *id)
}

// ---------------------------------------------------------------------------------------------
// runner
// ---------------------------------------------------------------------------------------------

type Check = fn(Case, &[u8]) -> Result<(), String>;

/// Runs `check` from each of `tier`'s starts, splitting `seeds` among the mixes and one
/// [`prop::budget`] among them all.
fn run(tier: Tier, seeds: u64, check: Check) {
    let began = Instant::now();
    let cases = (tier.starts.len() * MIXES.len()) as u32;
    let mut n = 0;
    for &start in tier.starts {
        for mix in MIXES {
            n += 1;
            let case = Case { tier: starts::population(tier, start), start, mix };
            let deadline = prop::budget().map(|budget| began + budget * n / cases);
            let paused = PAUSES.get();
            run_case(case, deadline, seeds / MIXES.len() as u64, check);
            if tier.overlap {
                assert!(
                    PAUSES.get() > paused,
                    "no step ran during a sync from {start:?} in {mix:?}"
                );
            }
        }
    }
}

thread_local! {
    /// How many steps this thread has taken while a sync waited on the server.
    static PAUSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn run_case(case: Case, deadline: Option<Instant>, seeds: u64, check: Check) {
    prop::run_until(
        deadline,
        seeds,
        160,
        |bytes| {
            std::panic::catch_unwind(|| check(case, bytes)).unwrap_or_else(|panic| {
                let message = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()));
                Err(format!("panicked: {}", message.unwrap_or_default()))
            })
        },
        |bytes| format!("from {:?} in {:?}\n{:#?}", case.start, case.mix, steps(case, bytes)),
    );
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

// ---------------------------------------------------------------------------------------------
// properties
// ---------------------------------------------------------------------------------------------

/// With nothing new on the server, a sync leaves the device as it was.
fn syncing_alone_changes_nothing_check(case: Case, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(case, bytes).await;
        for (n, step) in steps(case, bytes).iter().enumerate() {
            let step = step.confined(0);
            let answers = match &step {
                Step::SyncDuring { then, .. } => then.as_ref(),
                step => step,
            };
            if let Step::Accept { pending, .. } | Step::Reject { pending, .. } = *answers {
                // a share pending only on another device would be answered there
                if pick_pending(&world.devices[0], pending).await.is_none() {
                    continue;
                }
            }
            let before = tree(&world.devices[0]).await;
            let accepted = world.take(&step, 0, Reach::Shared).await;
            for (other, device) in world.devices.iter().enumerate().skip(1) {
                if !device.local_changes().await.is_empty() {
                    return Err(format!("step {n}: device {other} changed something"));
                }
            }
            if matches!(step, Step::Sync { .. } | Step::SyncAll { .. }) {
                if !accepted {
                    return Err(format!("step {n}: sync failed"));
                }
                let after = tree(&world.devices[0]).await;
                if after != before {
                    return Err(format!("step {n}: sync changed\n{before:#?}\ninto\n{after:#?}"));
                }
            }
        }
        Ok(())
    })
}

/// Devices that each touch only their own files leave every account where its first device taking
/// all of the account's steps, device by device, leaves it. Answers to shares are left out, since
/// whether a share is waiting depends on when the device last synced.
fn disjoint_changes_equal_sequential_check(case: Case, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let steps: Vec<_> = steps(case, bytes)
            .into_iter()
            .filter(|step| {
                let answers = match step {
                    Step::SyncDuring { then, .. } => then.as_ref(),
                    step => step,
                };
                !matches!(answers, Step::Accept { .. } | Step::Reject { .. })
            })
            .collect();

        let concurrent = World::new(case, bytes).await;
        for step in &steps {
            concurrent
                .take(step, step.device(), Reach::Own(step.device()))
                .await;
        }
        concurrent.settle().await?;

        let sequential = World::new(case, bytes).await;
        let per_account = sequential.per_account;
        for device in 0..sequential.devices.len() {
            let first = device / per_account * per_account;
            for step in steps
                .iter()
                .map(|s| match s {
                    Step::SyncDuring { then, .. } => then,
                    s => s,
                })
                .filter(|s| {
                    s.device() == device && !matches!(s, Step::Sync { .. } | Step::SyncAll { .. })
                })
            {
                sequential.take(step, first, Reach::Own(device)).await;
            }
        }
        sequential.settle().await?;

        for first in (0..sequential.devices.len()).step_by(per_account) {
            let merged = tree(&concurrent.devices[first]).await;
            let expected = tree(&sequential.devices[first]).await;
            if merged != expected {
                return Err(format!(
                    "device {first} merged\n{merged:#?}\nbut one after another gives\n{expected:#?}"
                ));
            }
        }
        Ok(())
    })
}

/// A device imported after a sync shows exactly what each account's syncing device shows.
fn a_new_device_sees_what_was_synced_check(case: Case, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(case, bytes).await;
        world.take_all(&steps(case, bytes)).await;
        world.settle().await?;
        for account in 0..case.tier.accounts {
            let synced = &world.account_devices(account)[0];
            let key = synced.export_account_private_key().unwrap();
            let fresh = device(&world.server, seed(bytes), world.devices.len() + 1 + account).await;
            fresh.import_account(&key, None).await.unwrap();
            fresh
                .sync()
                .await
                .map_err(|err| format!("account {account}'s new device failed to sync: {err:?}"))?;
            let synced = tree(synced).await;
            let seen = tree(&fresh).await;
            if seen != synced {
                return Err(format!(
                    "account {account}'s new device shows\n{seen:#?}\nbut its synced one \
                     shows\n{synced:#?}"
                ));
            }
        }
        Ok(())
    })
}

/// A step a device refuses leaves that device as it was.
fn a_refused_step_changes_nothing_check(case: Case, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(case, bytes).await;
        for (n, step) in steps(case, bytes).iter().enumerate() {
            let on = step.device();
            let before = tree(&world.devices[on]).await;
            let accepted = world.take(step, on, Reach::Shared).await;
            let after = tree(&world.devices[on]).await;
            if !accepted
                && !matches!(
                    step,
                    Step::Sync { .. } | Step::SyncAll { .. } | Step::SyncDuring { .. }
                )
                && after != before
            {
                return Err(format!(
                    "step {n} was refused but changed device {on}\n{before:#?}\ninto\n{after:#?}"
                ));
            }
        }
        Ok(())
    })
}

/// Once every device has synced, another sync leaves each device as it was.
fn a_second_sync_changes_nothing_check(case: Case, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(case, bytes).await;
        world.take_all(&steps(case, bytes)).await;
        world.settle().await?;
        for (i, device) in world.devices.iter().enumerate() {
            let before = tree(device).await;
            device
                .sync()
                .await
                .map_err(|err| format!("device {i}: {err:?}"))?;
            let after = tree(device).await;
            if after != before {
                return Err(format!(
                    "device {i} changed on a sync after settling\n{before:#?}\ninto\n{after:#?}"
                ));
            }
            if !device.local_changes().await.is_empty() {
                return Err(format!("device {i} has unpushed changes after settling"));
            }
        }
        Ok(())
    })
}

/// Once every device has synced, each account's devices show the same files.
fn devices_converge_check(case: Case, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(case, bytes).await;
        world.take_all(&steps(case, bytes)).await;
        world.settle().await?;
        for account in 0..case.tier.accounts {
            let devices = world.account_devices(account);
            let first = tree(&devices[0]).await;
            for (i, device) in devices.iter().enumerate().skip(1) {
                let other = tree(device).await;
                if other != first {
                    return Err(format!(
                        "account {account} device {i} shows\n{other:#?}\nbut device 0 \
                         shows\n{first:#?}"
                    ));
                }
            }
        }
        Ok(())
    })
}

/// Once every device has synced, a document two accounts both list has the same content for
/// both.
fn accounts_agree_on_what_they_share_check(case: Case, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(case, bytes).await;
        world.take_all(&steps(case, bytes)).await;
        world.settle().await?;
        let mut seen: Vec<BTreeMap<Uuid, String>> = Vec::new();
        for account in 0..case.tier.accounts {
            seen.push(documents(&world.account_devices(account)[0]).await);
        }
        for a in 0..case.tier.accounts {
            for b in a + 1..case.tier.accounts {
                for (id, content) in &seen[a] {
                    if let Some(other) = seen[b].get(id) {
                        if other != content {
                            return Err(format!(
                                "account {a} reads {content} but account {b} reads {other}"
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    })
}

/// Once every device has synced, the server's own consistency check finds nothing.
fn the_server_stays_valid_check(case: Case, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(case, bytes).await;
        world.take_all(&steps(case, bytes)).await;
        world.settle().await?;
        world.validate_server().await
    })
}
