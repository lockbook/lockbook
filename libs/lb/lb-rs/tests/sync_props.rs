//! Byte-stream properties of sync: devices on accounts take random steps against one in-memory
//! server, and each property holds at the widest tier whose steps it survives.
#![cfg(feature = "in-process-tests")]

use std::collections::BTreeMap;
use std::fmt::Write;

use lb_rs::io::server::Server;
use lb_rs::model::account::Account;
use lb_rs::model::api::{AdminValidateServer, AdminValidateServerRequest};
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
    run(SOLO, 200, syncing_alone_changes_nothing_check);
}

#[test]
fn a_new_device_sees_what_was_synced() {
    run(SOLO, 200, a_new_device_sees_what_was_synced_check);
}

#[test]
fn a_refused_step_changes_nothing() {
    run(DEVICES, 200, a_refused_step_changes_nothing_check);
}

#[test]
fn a_second_sync_changes_nothing() {
    run(DEVICES, 200, a_second_sync_changes_nothing_check);
}

#[test]
fn devices_converge() {
    run(DEVICES, 200, devices_converge_check);
}

#[test]
fn disjoint_changes_equal_sequential() {
    run(DEVICES, 200, disjoint_changes_equal_sequential_check);
}

#[test]
fn the_server_stays_valid() {
    run(DEVICES, 200, the_server_stays_valid_check);
}

// ---------------------------------------------------------------------------------------------
// tiers
// ---------------------------------------------------------------------------------------------

/// The steps a case may take. A property runs at the widest tier whose steps it survives.
#[derive(Clone, Copy, Debug)]
struct Tier {
    devices: usize,
}

/// One device on one account.
const SOLO: Tier = Tier { devices: 1 };

/// Several devices on one account, each syncing when it likes.
const DEVICES: Tier = Tier { devices: 3 };

// ---------------------------------------------------------------------------------------------
// steps
// ---------------------------------------------------------------------------------------------

/// Ordered boring to exotic: an exhausted buffer picks the first. Indexes pick among the
/// device's files sorted by path, so a buffer names the same files on every run.
#[derive(Clone, Debug)]
enum Step {
    Sync { device: usize },
    CreateDocument { device: usize, parent: usize, name: usize },
    CreateFolder { device: usize, parent: usize, name: usize },
    Write { device: usize, file: usize, content: usize },
    Rename { device: usize, file: usize, name: usize },
    Move { device: usize, file: usize, parent: usize },
    Delete { device: usize, file: usize },
}

// few names, so devices collide on paths
const NAMES: [&str; 3] = ["a.md", "b.md", "c"];
const CONTENTS: [&str; 3] = ["one", "two\nlines", ""];

fn steps(tier: Tier, bytes: &[u8]) -> Vec<Step> {
    let mut src = ByteSource::new(bytes);
    let count = 1 + src.draw(16);
    (0..count)
        .map(|_| {
            let device = src.draw(tier.devices);
            match src.bias(&[3, 3, 2, 3, 2, 2, 1]) {
                0 => Step::Sync { device },
                1 => Step::CreateDocument { device, parent: src.draw(8), name: src.draw(3) },
                2 => Step::CreateFolder { device, parent: src.draw(8), name: src.draw(3) },
                3 => Step::Write { device, file: src.draw(8), content: src.draw(3) },
                4 => Step::Rename { device, file: src.draw(8), name: src.draw(3) },
                5 => Step::Move { device, file: src.draw(8), parent: src.draw(8) },
                _ => Step::Delete { device, file: src.draw(8) },
            }
        })
        .collect()
}

impl Step {
    fn device(&self) -> usize {
        match *self {
            Step::Sync { device }
            | Step::CreateDocument { device, .. }
            | Step::CreateFolder { device, .. }
            | Step::Write { device, .. }
            | Step::Rename { device, .. }
            | Step::Move { device, .. }
            | Step::Delete { device, .. } => device,
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

/// One account's devices, an admin, and the server they share.
struct World {
    server: InMemoryServer,
    admin: Account,
    devices: Vec<Lb>,
}

impl World {
    async fn new(devices: usize) -> Self {
        let mut server = InMemoryServer::in_memory();
        let admin_name = random_name();
        server.config.admin.admins.insert(admin_name.clone());
        let admin = device(&server).await;
        let admin = admin.create_account(&admin_name, "", false).await.unwrap();

        let first = device(&server).await;
        first
            .create_account(&random_name(), "", false)
            .await
            .unwrap();
        let key = first.export_account_private_key().unwrap();
        let mut all = vec![first];
        for _ in 1..devices {
            let next = device(&server).await;
            next.import_account(&key, None).await.unwrap();
            next.sync().await.unwrap();
            all.push(next);
        }
        Self { server, admin, devices: all }
    }

    /// Takes `step` on device `on`, returning whether the device accepted it.
    async fn take(&self, step: &Step, on: usize, reach: Reach) -> bool {
        let device = &self.devices[on];
        match *step {
            Step::Sync { .. } => device.sync().await.is_ok(),
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
                device
                    .write_document(file, CONTENTS[content].as_bytes())
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
        }
    }

    /// Takes every step on the device it names.
    async fn take_all(&self, steps: &[Step]) {
        for step in steps {
            self.take(step, step.device(), Reach::Shared).await;
        }
    }

    /// Syncs every device until each has seen every other's changes.
    async fn settle(&self) -> Result<(), String> {
        for _ in 0..2 {
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
    Lb::with_server(test_config(), server.clone())
        .await
        .unwrap()
}

/// What a device shows: every file's type and content, keyed by path.
async fn tree(device: &Lb) -> BTreeMap<String, String> {
    let mut tree = BTreeMap::new();
    for file in device.list_metadatas().await.unwrap() {
        let mut shown = format!("{:?}", file.file_type);
        if file.is_document() {
            let content = device.read_document(file.id, false).await.unwrap();
            write!(shown, " {:?}", String::from_utf8_lossy(&content)).unwrap();
        }
        tree.insert(device.get_path_by_id(file.id).await.unwrap(), shown);
    }
    tree
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Any,
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

// ---------------------------------------------------------------------------------------------
// runner
// ---------------------------------------------------------------------------------------------

type Check = fn(Tier, &[u8]) -> Result<(), String>;

fn run(tier: Tier, seeds: u64, check: Check) {
    prop::run(
        seeds,
        64,
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
        let world = World::new(tier.devices).await;
        for (n, step) in steps(tier, bytes).iter().enumerate() {
            let before = tree(&world.devices[0]).await;
            let accepted = world.take(step, 0, Reach::Shared).await;
            if matches!(step, Step::Sync { .. }) {
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

/// A device imported after a sync shows exactly what the syncing device shows.
fn a_new_device_sees_what_was_synced_check(tier: Tier, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(tier.devices).await;
        world.take_all(&steps(tier, bytes)).await;
        world.settle().await?;
        let key = world.devices[0].export_account_private_key().unwrap();
        let fresh = device(&world.server).await;
        fresh.import_account(&key, None).await.unwrap();
        fresh
            .sync()
            .await
            .map_err(|err| format!("new device failed to sync: {err:?}"))?;
        if tree(&fresh).await != tree(&world.devices[0]).await {
            return Err("new device differs from the device that synced".into());
        }
        Ok(())
    })
}

/// A step a device refuses leaves that device as it was.
fn a_refused_step_changes_nothing_check(tier: Tier, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(tier.devices).await;
        for (n, step) in steps(tier, bytes).iter().enumerate() {
            let on = step.device();
            let before = tree(&world.devices[on]).await;
            let accepted = world.take(step, on, Reach::Shared).await;
            if !accepted
                && !matches!(step, Step::Sync { .. })
                && tree(&world.devices[on]).await != before
            {
                return Err(format!("step {n} was refused but changed device {on}"));
            }
        }
        Ok(())
    })
}

/// Once every device has synced twice, another sync leaves each device as it was.
fn a_second_sync_changes_nothing_check(tier: Tier, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(tier.devices).await;
        world.take_all(&steps(tier, bytes)).await;
        world.settle().await?;
        for (i, device) in world.devices.iter().enumerate() {
            let before = tree(device).await;
            device
                .sync()
                .await
                .map_err(|err| format!("device {i}: {err:?}"))?;
            if tree(device).await != before {
                return Err(format!("device {i} changed on a sync after settling"));
            }
            if !device.local_changes().await.is_empty() {
                return Err(format!("device {i} has unpushed changes after settling"));
            }
        }
        Ok(())
    })
}

/// Once every device has synced twice, every device shows the same files.
fn devices_converge_check(tier: Tier, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(tier.devices).await;
        world.take_all(&steps(tier, bytes)).await;
        world.settle().await?;
        let first = tree(&world.devices[0]).await;
        for (i, device) in world.devices.iter().enumerate().skip(1) {
            let other = tree(device).await;
            if other != first {
                return Err(format!(
                    "device {i} shows\n{other:#?}\nbut device 0 shows\n{first:#?}"
                ));
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

        let concurrent = World::new(tier.devices).await;
        for step in &steps {
            concurrent
                .take(step, step.device(), Reach::Own(step.device()))
                .await;
        }
        concurrent.settle().await?;

        let sequential = World::new(1).await;
        for device in 0..tier.devices {
            for step in steps
                .iter()
                .filter(|s| s.device() == device && !matches!(s, Step::Sync { .. }))
            {
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

/// The server's own consistency check finds nothing after every step.
fn the_server_stays_valid_check(tier: Tier, bytes: &[u8]) -> Result<(), String> {
    block_on(async {
        let world = World::new(tier.devices).await;
        for (n, step) in steps(tier, bytes).iter().enumerate() {
            world.take(step, step.device(), Reach::Shared).await;
            world
                .validate_server()
                .await
                .map_err(|err| format!("after step {n}: {err}"))?;
        }
        world.settle().await?;
        world.validate_server().await
    })
}
