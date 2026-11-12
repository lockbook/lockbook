//! Each start is built once per test binary, from a seed of its own, and every case gets its own
//! copy of the server and of every device.

use std::sync::Mutex;

use super::*;

/// Friends planning a trip, each with two devices.
const VACATION: Tier = Tier { accounts: 4, devices: 2, ..OVERLAP };

/// The accounts and devices a case from `start` has: the start's own, or else `tier`'s.
pub(super) fn population(tier: Tier, start: Start) -> Tier {
    match start {
        Start::Vacation => Tier { starts: tier.starts, overlap: tier.overlap, ..VACATION },
        Start::Empty | Start::Shared | Start::WriteShared => tier,
    }
}

/// The world a case at `tier` from `start` copies, built the first time it is asked for.
pub(super) fn built(tier: Tier, start: Start) -> &'static World {
    type Built = ((usize, usize, Start), &'static World);
    static BUILT: Mutex<Vec<Built>> = Mutex::new(Vec::new());
    let key = (tier.accounts, tier.devices, start);
    let mut built = BUILT.lock().unwrap();
    if let Some((_, world)) = built.iter().find(|(k, _)| *k == key) {
        return world;
    }
    // on its own thread and runtime, since this one's runtime is waiting on it
    let world = std::thread::scope(|scope| {
        scope
            .spawn(|| block_on(build(tier, start)))
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    });
    let world: &'static World = Box::leak(Box::new(world));
    built.push((key, world));
    world
}

async fn build(tier: Tier, start: Start) -> World {
    let seed = super::seed(format!("{start:?} {} {}", tier.accounts, tier.devices).as_bytes());
    let world = World::empty(tier, seed).await;
    match start {
        Start::Empty => {}
        Start::Shared => world.start_shared(1, ShareMode::Read).await,
        Start::WriteShared => world.start_shared(3, ShareMode::Write).await,
        Start::Vacation => vacation(&world).await,
    }
    world
}

impl World {
    fn first(&self, account: usize) -> &Lb {
        &self.account_devices(account)[0]
    }
}

async fn create_at(device: &Lb, path: &str) -> Uuid {
    device.create_at_path(path).await.unwrap().id
}

async fn write(device: &Lb, id: Uuid, content: &str) {
    device.write_document(id, content.as_bytes()).await.unwrap();
}

async fn share(device: &Lb, id: Uuid, with: &str, mode: ShareMode) {
    device.share_file(id, with, mode).await.unwrap();
}

async fn sync(device: &Lb) {
    device.sync().await.unwrap();
}

/// Accepts the share of `target` waiting for `device` as a link at `path`.
async fn accept(device: &Lb, path: &str, target: Uuid) {
    device.create_link_at_path(path, target).await.unwrap();
}

/// Four friends share a trip folder for writing. Everyone has edited the itinerary in turn, and
/// one friend's second device has not synced since it signed in.
async fn vacation(world: &World) {
    let organizer = world.first(0);
    let trip = create_at(organizer, "Trip/").await;
    let itinerary = create_at(organizer, "Trip/itinerary.md").await;
    write(organizer, itinerary, "# Lisbon\nday 1: arrive\n").await;
    let packing = create_at(organizer, "Trip/packing.md").await;
    write(organizer, packing, "- passport\n").await;
    create_at(organizer, "Trip/flights/outbound.md").await;
    create_at(organizer, "Trip/flights/return.md").await;
    create_at(organizer, "Notes/gifts.md").await;
    for friend in 1..4 {
        share(organizer, trip, &world.names[friend], ShareMode::Write).await;
    }
    sync(organizer).await;

    let links = ["Trip", "Shared/Trip", "Trip"];
    let mut plan = String::from("# Lisbon\nday 1: arrive\n");
    for friend in 1..4 {
        let device = world.first(friend);
        sync(device).await;
        accept(device, links[friend - 1], trip).await;
        create_at(device, "Notes/todo.md").await;
        writeln!(plan, "day {}: friend {friend}'s pick", friend + 1).unwrap();
        write(device, itinerary, &plan).await;
        sync(device).await;
    }
    let photos = world.first(1);
    let photo_list = photos
        .create_file("photos", &trip, FileType::Folder)
        .await
        .unwrap();
    photos
        .create_file("beach.md", &photo_list.id, FileType::Document)
        .await
        .unwrap();
    sync(photos).await;

    // the organizer's laptop adds to the packing list
    let laptop = &world.account_devices(0)[1];
    sync(laptop).await;
    write(laptop, packing, "- passport\n- sunscreen\n").await;
    sync(laptop).await;

    let rarely_syncs = &world.account_devices(3)[1];
    for _ in 0..2 {
        for device in world
            .devices
            .iter()
            .filter(|device| !std::ptr::eq(*device, rarely_syncs))
        {
            sync(device).await;
        }
    }
}

/// A copy shows what its original shows, and steps on the copy leave the original as it was.
#[test]
fn a_copy_diverges_from_its_original() {
    for start in [Start::Shared, Start::Vacation] {
        block_on(async {
            let original = built(population(SHARES, start), start);
            let before = trees(original).await;
            let copy = original.copy(0).await;
            assert_eq!(trees(&copy).await, before, "{start:?} copied");

            let device = &copy.devices[1];
            let root = device.root().await.unwrap().id;
            let doc = device
                .create_file("new.md", &root, FileType::Document)
                .await
                .unwrap();
            device.write_document(doc.id, b"diverged").await.unwrap();
            copy.settle().await.unwrap();
            copy.validate_server().await.unwrap();
            assert_ne!(trees(&copy).await, before, "{start:?} copy diverged");
            assert_eq!(trees(original).await, before, "{start:?} original untouched");
            original.validate_server().await.unwrap();
        });
    }
}

/// The same seed draws the same ids on every copy, so a failing case replays.
#[test]
fn a_copy_draws_ids_from_its_seed() {
    block_on(async {
        let original = built(SHARES, Start::Shared);
        let mut ids = Vec::new();
        for _ in 0..2 {
            let copy = original.copy(7).await;
            let made = copy.devices[2].create_at_path("x/y.md").await.unwrap();
            ids.push((made.id, made.parent));
        }
        assert_eq!(ids[0], ids[1]);
    });
}

async fn trees(world: &World) -> Vec<BTreeMap<String, String>> {
    let mut trees = Vec::new();
    for device in &world.devices {
        trees.push(tree(device).await);
    }
    trees
}
