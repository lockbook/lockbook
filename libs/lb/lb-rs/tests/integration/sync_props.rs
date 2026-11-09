//! Byte-stream properties of sync: devices on accounts take random steps against one in-memory
//! server, and each property holds at the widest tier whose steps it survives.

use std::collections::BTreeMap;
use std::fmt::Write;

use lb_rs::io::server::Server;
use lb_rs::model::account::Account;
use lb_rs::model::api::{AdminValidateServer, AdminValidateServerRequest};
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::{File, ShareMode};
use lb_rs::model::file_metadata::FileType;
use test_utils::byte_source::ByteSource;
use test_utils::lockbook_server_lib::in_process::InMemoryServer;
use test_utils::{Lb, prop, random_name, test_config};
use uuid::Uuid;

// ---------------------------------------------------------------------------------------------
// property table
// ---------------------------------------------------------------------------------------------

#[test]
fn syncing_alone_changes_nothing() {
    run(SOLO, 2000, syncing_alone_changes_nothing_check);
}

#[test]
fn disjoint_changes_equal_sequential() {
    run(DEVICES, 2000, disjoint_changes_equal_sequential_check);
}

#[test]
fn a_new_device_sees_what_was_synced() {
    run(SHARES, 2000, a_new_device_sees_what_was_synced_check);
}

#[test]
fn a_refused_step_changes_nothing() {
    run(SHARES, 2000, a_refused_step_changes_nothing_check);
}

#[test]
fn a_second_sync_changes_nothing() {
    run(SHARES, 2000, a_second_sync_changes_nothing_check);
}

#[test]
fn devices_converge() {
    run(SHARES, 2000, devices_converge_check);
}

#[test]
fn accounts_agree_on_what_they_share() {
    run(SHARES, 2000, accounts_agree_on_what_they_share_check);
}

#[test]
fn the_server_stays_valid() {
    run(SHARES, 2000, the_server_stays_valid_check);
}

// ---------------------------------------------------------------------------------------------
// tiers
// ---------------------------------------------------------------------------------------------

/// The steps a case may take. A property runs at the widest tier whose steps it survives.
#[derive(Clone, Copy, Debug)]
struct Tier {
    accounts: usize,
    devices: usize,
}

/// One device on one account.
const SOLO: Tier = Tier { accounts: 1, devices: 1 };

/// Several devices on one account, each syncing when it likes.
const DEVICES: Tier = Tier { accounts: 1, devices: 3 };

/// Several accounts with two devices each, sharing with one another.
const SHARES: Tier = Tier { accounts: 3, devices: 2 };

impl Tier {
    fn all_devices(self) -> usize {
        self.accounts * self.devices
    }
}

// ---------------------------------------------------------------------------------------------
// steps
// ---------------------------------------------------------------------------------------------

/// Ordered boring to exotic: an exhausted buffer picks the first. Indexes pick among the
/// device's files sorted by path, so a buffer names the same files on every run.
#[derive(Clone, Debug)]
enum Step {
    Sync { device: usize },
    SyncAll { device: usize },
    CreateDocument { device: usize, parent: usize, name: usize },
    CreateFolder { device: usize, parent: usize, name: usize },
    Write { device: usize, file: usize, content: usize },
    Rename { device: usize, file: usize, name: usize },
    Move { device: usize, file: usize, parent: usize },
    Delete { device: usize, file: usize },
    Share { device: usize, file: usize, to: usize, write: bool },
    Accept { device: usize, pending: usize, name: usize },
    Reject { device: usize, pending: usize },
    Unshare { device: usize, file: usize, from: usize },
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

fn steps(tier: Tier, bytes: &[u8]) -> Vec<Step> {
    let mut src = ByteSource::new(bytes);
    let count = 1 + src.draw(48);
    let shares = if tier.accounts > 1 { 2 } else { 0 };
    let mut device = 0;
    (0..count)
        .map(|_| {
            // people work on one device for a while
            if src.bias(&[4, 1]) == 1 {
                device = src.draw(tier.all_devices());
            }
            match src.bias(&[3, 1, 3, 2, 3, 2, 2, 1, shares, shares, shares, shares]) {
                0 => Step::Sync { device },
                1 => Step::SyncAll { device },
                2 => Step::CreateDocument {
                    device,
                    parent: src.draw(8),
                    name: src.bias(&NAME_WEIGHTS),
                },
                3 => Step::CreateFolder {
                    device,
                    parent: src.draw(8),
                    name: src.bias(&NAME_WEIGHTS),
                },
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
        })
        .collect()
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
            | Step::Unshare { device, .. } => device,
        }
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
    server: InMemoryServer,
    admin: Account,
    names: Vec<String>,
    devices: Vec<Lb>,
    per_account: usize,
}

impl World {
    async fn new(tier: Tier) -> Self {
        let mut server = InMemoryServer::in_memory();
        let admin_name = random_name();
        server.config.admin.admins.insert(admin_name.clone());
        let admin = device(&server).await;
        let admin = admin.create_account(&admin_name, "", false).await.unwrap();

        let mut names = Vec::new();
        let mut devices = Vec::new();
        for _ in 0..tier.accounts {
            let name = random_name();
            let first = device(&server).await;
            first.create_account(&name, "", false).await.unwrap();
            let key = first.export_account_private_key().unwrap();
            devices.push(first);
            for _ in 1..tier.devices {
                let next = device(&server).await;
                next.import_account(&key, None).await.unwrap();
                next.sync().await.unwrap();
                devices.push(next);
            }
            names.push(name);
        }
        Self { server, admin, names, devices, per_account: tier.devices }
    }

    fn account_devices(&self, account: usize) -> &[Lb] {
        &self.devices[account * self.per_account..(account + 1) * self.per_account]
    }

    /// Takes `step` on device `on`, returning whether the device accepted it.
    async fn take(&self, step: &Step, on: usize, reach: Reach) -> bool {
        let device = &self.devices[on];
        match *step {
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

async fn device(server: &InMemoryServer) -> Lb {
    Lb::in_memory(test_config(), server.clone()).await.unwrap()
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

type Check = fn(Tier, &[u8]) -> Result<(), String>;

fn run(tier: Tier, seeds: u64, check: Check) {
    prop::run(
        seeds,
        160,
        |bytes| {
            std::panic::catch_unwind(|| check(tier, bytes)).unwrap_or_else(|panic| {
                let message = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()));
                Err(format!("panicked: {}", message.unwrap_or_default()))
            })
        },
        |bytes| format!("{:#?}", steps(tier, bytes)),
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
fn syncing_alone_changes_nothing_check(tier: Tier, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(tier).await;
        for (n, step) in steps(tier, bytes).iter().enumerate() {
            let before = tree(&world.devices[0]).await;
            let accepted = world.take(step, 0, Reach::Shared).await;
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

/// Devices that each touch only their own files end where one device taking all their steps,
/// device by device, ends.
fn disjoint_changes_equal_sequential_check(tier: Tier, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let steps = steps(tier, bytes);

        let concurrent = World::new(tier).await;
        for step in &steps {
            concurrent
                .take(step, step.device(), Reach::Own(step.device()))
                .await;
        }
        concurrent.settle().await?;

        let sequential = World::new(SOLO).await;
        for device in 0..tier.all_devices() {
            for step in steps.iter().filter(|s| {
                s.device() == device && !matches!(s, Step::Sync { .. } | Step::SyncAll { .. })
            }) {
                sequential.take(step, 0, Reach::Own(device)).await;
            }
        }

        let merged = tree(&concurrent.devices[0]).await;
        let expected = tree(&sequential.devices[0]).await;
        if merged != expected {
            return Err(format!("merged\n{merged:#?}\nbut one after another gives\n{expected:#?}"));
        }
        Ok(())
    })
}

/// A device imported after a sync shows exactly what the syncing device shows.
fn a_new_device_sees_what_was_synced_check(tier: Tier, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(tier).await;
        world.take_all(&steps(tier, bytes)).await;
        world.settle().await?;
        let key = world.devices[0].export_account_private_key().unwrap();
        let fresh = device(&world.server).await;
        fresh.import_account(&key, None).await.unwrap();
        fresh
            .sync()
            .await
            .map_err(|err| format!("new device failed to sync: {err:?}"))?;
        let synced = tree(&world.devices[0]).await;
        let seen = tree(&fresh).await;
        if seen != synced {
            return Err(format!(
                "new device shows\n{seen:#?}\nbut the synced one shows\n{synced:#?}"
            ));
        }
        Ok(())
    })
}

/// A step a device refuses leaves that device as it was.
fn a_refused_step_changes_nothing_check(tier: Tier, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(tier).await;
        for (n, step) in steps(tier, bytes).iter().enumerate() {
            let on = step.device();
            let before = tree(&world.devices[on]).await;
            let accepted = world.take(step, on, Reach::Shared).await;
            let after = tree(&world.devices[on]).await;
            if !accepted
                && !matches!(step, Step::Sync { .. } | Step::SyncAll { .. })
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
fn a_second_sync_changes_nothing_check(tier: Tier, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(tier).await;
        world.take_all(&steps(tier, bytes)).await;
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
fn devices_converge_check(tier: Tier, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(tier).await;
        world.take_all(&steps(tier, bytes)).await;
        world.settle().await?;
        for account in 0..tier.accounts {
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
fn accounts_agree_on_what_they_share_check(tier: Tier, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(tier).await;
        world.take_all(&steps(tier, bytes)).await;
        world.settle().await?;
        let mut seen: Vec<BTreeMap<Uuid, String>> = Vec::new();
        for account in 0..tier.accounts {
            seen.push(documents(&world.account_devices(account)[0]).await);
        }
        for a in 0..tier.accounts {
            for b in a + 1..tier.accounts {
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
fn the_server_stays_valid_check(tier: Tier, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(tier).await;
        world.take_all(&steps(tier, bytes)).await;
        world.settle().await?;
        world.validate_server().await
    })
}
