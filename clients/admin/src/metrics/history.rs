use super::{Error, Result};
use db_rs::views::composite_view::Schema;
use lb::Uuid;
use lb::model::file_like::FileLike;
use lb::model::file_metadata::Owner;
use lb::model::server_meta::ServerMeta;
use lb::model::server_tree::ServerTree;
use lb::model::tree_like::TreeLike;
use lockbook_server_lib::billing::billing_model::BillingPlatform;
use lockbook_server_lib::schema::{Account, ServerV6};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{self, Read, Seek};

pub const DAY: i64 = 86_400_000;
pub const HOUR: i64 = 3_600_000;
const WELCOME_DELAY: i64 = 5_000;

#[derive(Deserialize)]
pub enum Entry<'a> {
    Events {
        seq_no: u64,
        #[serde(borrow)]
        payload: &'a [u8],
    },
    Snapshot,
}

#[derive(Deserialize)]
struct TableEvent<'a> {
    table_id: usize,
    last_modified: Option<u64>,
    #[serde(borrow)]
    payload: &'a [u8],
}

#[derive(Deserialize)]
enum MapChange<K, V> {
    Insert { key: K, value: V },
    Remove { key: K },
    Clear,
}

#[derive(Deserialize)]
enum SetChange<K, V> {
    Insert { key: K, value: V },
    Remove { key: K, value: V },
    CreateKey { key: K },
    ClearKey { key: K },
    Clear,
}

pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T> {
    let (value, read) =
        bincode::serde::borrow_decode_from_slice(bytes, bincode::config::standard())?;
    if read != bytes.len() {
        return Err("unexpected bytes at the end of a db-rs record".into());
    }
    Ok(value)
}

pub enum Frame {
    Complete(Vec<u8>),
    End,
    Partial,
}

pub fn read_frame(reader: &mut (impl Read + Seek), end: u64) -> Result<Frame> {
    let remaining = end.saturating_sub(reader.stream_position()?);
    if remaining == 0 {
        return Ok(Frame::End);
    }
    if remaining < 8 {
        return Ok(Frame::Partial);
    }
    let mut size = [0; 8];
    reader.read_exact(&mut size)?;
    let size = u64::from_be_bytes(size);
    if size > remaining - 8 {
        return Ok(Frame::Partial);
    }
    let size = usize::try_from(size)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(size)?;
    bytes.resize(size, 0);
    reader.read_exact(&mut bytes)?;
    Ok(Frame::Complete(bytes))
}

fn frames(mut bytes: &[u8], mut f: impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
    while let Some(entry) = db_rs::payload_buffer::PayloadBuffer::head_payload(&mut bytes)? {
        f(entry)?;
    }
    Ok(())
}

fn map_changes<K: DeserializeOwned, V: DeserializeOwned>(
    bytes: &[u8], mut change: impl FnMut(Option<(K, Option<V>)>),
) -> Result<()> {
    frames(bytes, |bytes| {
        change(match decode::<MapChange<K, V>>(bytes)? {
            MapChange::Insert { key, value } => Some((key, Some(value))),
            MapChange::Remove { key } => Some((key, None)),
            MapChange::Clear => None,
        });
        Ok(())
    })
}

fn set_keys<K: DeserializeOwned, V: DeserializeOwned>(
    bytes: &[u8], mut change: impl FnMut(Option<K>),
) -> Result<()> {
    frames(bytes, |bytes| {
        change(match decode::<SetChange<K, V>>(bytes)? {
            SetChange::Insert { key, value } | SetChange::Remove { key, value } => {
                drop(value);
                Some(key)
            }
            SetChange::CreateKey { key } | SetChange::ClearKey { key } => Some(key),
            SetChange::Clear => None,
        });
        Ok(())
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provider {
    Stripe,
    Play,
    AppStore,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountState {
    pub live: bool,
    pub files: usize,
    pub storage: Option<u64>,
    pub shared: bool,
    pub provider: Option<Provider>,
    pub egress: u64,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct AccountHistory {
    pub(super) id: [u8; 8],
    pub(super) username: Option<String>,
    pub created: Option<i64>,
    pub observed_signup: bool,
    pub states: Vec<(u64, AccountState)>,
    pub activity: BTreeSet<i64>,
}

impl AccountHistory {
    pub fn display_name(&self) -> Cow<'_, str> {
        self.username.as_deref().map_or_else(
            || Cow::Owned(format!("Account {:016x}", u64::from_be_bytes(self.id))),
            Cow::Borrowed,
        )
    }
    pub fn at(&self, sequence: u64) -> Option<&AccountState> {
        self.states
            .get(
                self.states
                    .partition_point(|(seq, _)| *seq <= sequence)
                    .checked_sub(1)?,
            )
            .map(|(_, state)| state)
    }

    pub fn active(&self, start: i64, end: i64) -> bool {
        self.activity
            .range(start..)
            .next()
            .is_some_and(|activity| *activity <= end)
    }

    pub fn active_hours(&self, start: i64, end: i64) -> f64 {
        let mut covered_until = start;
        let mut duration = 0;
        for &event in self.activity.range(start.saturating_sub(HOUR)..=end) {
            let window_end = event.saturating_add(HOUR).min(end);
            duration += (window_end - event.max(covered_until)).max(0);
            covered_until = covered_until.max(window_end);
        }
        duration as f64 / HOUR as f64
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub users: usize,
    pub premium: [usize; 3],
    pub shared: usize,
    pub files: usize,
    pub storage: u64,
    pub unknown_storage: usize,
}

impl Checkpoint {
    fn change(&mut self, row: &AccountState, insert: bool) {
        if !row.live {
            return;
        }
        fn add(value: &mut usize, amount: usize, insert: bool) {
            if insert {
                *value += amount;
            } else {
                *value -= amount;
            }
        }
        add(&mut self.users, 1, insert);
        add(&mut self.shared, usize::from(row.shared), insert);
        add(&mut self.files, row.files, insert);
        if let Some(storage) = row.storage {
            if insert {
                self.storage += storage;
            } else {
                self.storage -= storage;
            }
        } else {
            add(&mut self.unknown_storage, 1, insert);
        }
        if let Some(provider) = row.provider {
            add(&mut self.premium[provider as usize], 1, insert);
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct History {
    // Table order, not a username, identifies a replayed account.
    pub accounts: Vec<AccountHistory>,
    pub checkpoints: Vec<(u64, Checkpoint)>,
    pub server_egress: Vec<(u64, u64)>,
    pub anchors: Vec<(i64, u64)>,
    pub gaps: Vec<(u64, u64)>,
    pub latest: u64,
    pub oldest: u64,
    pub partial_tail: bool,
    pub import_signature: Option<[u8; 32]>,
}

impl History {
    pub fn sequence_at(&self, time: i64) -> Option<u64> {
        let position = self.anchors.partition_point(|(stamp, _)| *stamp <= time);
        let &(before_time, before_sequence) = self.anchors.get(position.checked_sub(1)?)?;
        if before_time == time {
            return Some(before_sequence);
        }
        let &(after_time, after_sequence) = self.anchors.get(position)?;
        let elapsed = i128::from(time - before_time);
        let distance = i128::from(after_sequence - before_sequence);
        Some(before_sequence + (elapsed * distance / i128::from(after_time - before_time)) as u64)
    }

    pub fn timestamp_at(&self, sequence: u64) -> Option<i64> {
        let position = self.anchors.partition_point(|(_, seq)| *seq < sequence);
        let &(after_time, after_seq) = self.anchors.get(position)?;
        if after_seq == sequence {
            return Some(after_time);
        }
        let &(before_time, before_seq) = self.anchors.get(position.checked_sub(1)?)?;
        let elapsed = i128::from(after_time - before_time);
        let distance = i128::from(sequence - before_seq);
        Some(before_time + (elapsed * distance / i128::from(after_seq - before_seq)) as i64)
    }

    pub fn anchor(&mut self, time: i64, sequence: u64) {
        if time > 0
            && self
                .anchors
                .last()
                .is_none_or(|&(prior, seq)| time >= prior && sequence >= seq)
            && self.anchors.last() != Some(&(time, sequence))
        {
            self.anchors.push((time, sequence));
        }
    }

    pub fn covered(&self, start: i64, end: i64) -> bool {
        let Some(first) = self.sequence_at(start) else { return false };
        let Some(last) = self.sequence_at(end) else { return false };
        self.gaps
            .iter()
            .all(|&(left, right)| last <= left || first >= right)
    }

    pub fn sequence_covered(&self, seq: u64) -> bool {
        seq >= self.oldest
            && seq <= self.latest
            && self
                .gaps
                .iter()
                .all(|&(left, right)| seq <= left || seq >= right)
    }

    pub fn checkpoint(&self, seq: u64) -> Option<Checkpoint> {
        if !self.sequence_covered(seq) {
            return None;
        }
        Some(value_at(&self.checkpoints, seq))
    }
}

pub fn value_at<T: Copy + Default>(values: &[(u64, T)], sequence: u64) -> T {
    let index = values.partition_point(|(seq, _)| *seq <= sequence);
    index
        .checked_sub(1)
        .map(|i| values[i].1)
        .unwrap_or_default()
}

#[derive(Default)]
struct Changes {
    full: HashSet<Owner>,
    scalar: HashSet<Owner>,
    files: HashSet<Uuid>,
    owners_reset: bool,
    root_events: HashMap<Owner, i64>,
    activity: HashMap<Owner, i64>,
    time_hint: Option<i64>,
    server_egress: bool,
    initial_bulk_import: bool,
}

impl Changes {
    fn hint(&mut self, time: i64) {
        self.time_hint = Some(self.time_hint.map_or(time, |prior| prior.max(time)));
    }
}

#[derive(Default)]
pub struct Replay {
    pub history: History,
    db: ServerV6,
    users: HashMap<Owner, usize>,
    checkpoint: Checkpoint,
}

impl Replay {
    fn user(&mut self, owner: Owner) -> &mut AccountHistory {
        let index = *self.users.entry(owner).or_insert_with(|| {
            let index = self.history.accounts.len();
            let digest = Sha256::digest(owner.0.serialize_compressed());
            let mut id = [0; 8];
            id.copy_from_slice(&digest[..8]);
            self.history
                .accounts
                .push(AccountHistory { id, ..Default::default() });
            index
        });
        &mut self.history.accounts[index]
    }

    fn file_owners(&self, id: Uuid, owners: &mut HashSet<Owner>) {
        let mut id = id;
        let mut visited = HashSet::new();
        while visited.insert(id) {
            let Some(meta) = self.db.metas.get(&id) else { break };
            owners.insert(meta.owner());
            owners.extend(
                meta.user_access_keys()
                    .iter()
                    .filter(|key| !key.deleted)
                    .map(|key| Owner(key.encrypted_for)),
            );
            id = *meta.parent();
        }
    }

    pub fn apply(&mut self, seq: u64, payload: &[u8], baseline: bool) -> Result<()> {
        if baseline {
            self.db = ServerV6::default();
        }
        let mut changes =
            Changes { owners_reset: baseline, server_egress: baseline, ..Default::default() };
        // The order is the persisted ServerV6 Schema order. Skip tables that do
        // not contribute to metrics before they are decoded by db-rs.
        frames(payload, |bytes| {
            let event: TableEvent<'_> = decode(bytes)?;
            if seq == 1 && matches!(event.table_id, 2..=5 | 10..=13) {
                changes.initial_bulk_import = true;
            }
            match event.table_id {
                1 => map_changes::<Uuid, ServerMeta>(event.payload, |change| {
                    let Some((key, value)) = change else {
                        changes.owners_reset = true;
                        return;
                    };
                    self.file_owners(key, &mut changes.full);
                    changes.files.insert(key);
                    if let Some(meta) = value {
                        changes.full.insert(meta.owner());
                        if !baseline {
                            changes.hint(meta.version as i64);
                        }
                        if meta.is_root() {
                            changes
                                .root_events
                                .insert(meta.owner(), meta.file.timestamped_value.timestamp);
                        }
                    }
                })?,
                5 => map_changes::<Owner, u64>(event.payload, |change| {
                    if let Some((owner, Some(value))) = change {
                        if !baseline {
                            changes.activity.insert(owner, value as i64);
                            changes.hint(value as i64);
                        }
                    }
                })?,
                6 => map_changes::<Owner, Account>(event.payload, |change| {
                    if let Some((owner, _)) = change {
                        changes.full.insert(owner);
                    } else {
                        changes.owners_reset = true;
                    }
                })?,
                7 | 8 => set_keys::<Owner, Uuid>(event.payload, |owner| {
                    if let Some(owner) = owner {
                        changes.full.insert(owner);
                    } else {
                        changes.owners_reset = true;
                    }
                })?,
                9 => set_keys::<Uuid, Uuid>(event.payload, |id| {
                    if let Some(id) = id {
                        self.file_owners(id, &mut changes.full);
                        changes.files.insert(id);
                    } else {
                        changes.owners_reset = true;
                    }
                })?,
                10 => changes.server_egress = true,
                11 => map_changes::<Owner, lockbook_server_lib::defense::BandwidthReport>(
                    event.payload,
                    |value| {
                        if let Some((owner, _)) = value {
                            changes.scalar.insert(owner);
                        } else {
                            changes.owners_reset = true;
                        }
                    },
                )?,
                0 | 2..=4 | 12 | 13 => return Ok(()),
                other => return Err(format!("unknown ServerV6 table {other}").into()),
            }
            self.db.views_mut().as_mut()[event.table_id]
                .handle_events(event.last_modified.unwrap_or(seq), event.payload)?;
            Ok(())
        })?;
        for file in changes.files {
            self.file_owners(file, &mut changes.full);
        }
        if changes.owners_reset {
            changes
                .full
                .extend(self.db.accounts.iter().map(|(owner, _)| *owner));
            changes.full.extend(self.users.keys().copied());
        }
        let initial_bulk_import = seq == 1
            && (changes.initial_bulk_import
                || changes.root_events.len() > 1
                || changes.full.len() > 1);
        if initial_bulk_import {
            changes.time_hint = None;
        }
        for (owner, created) in changes.root_events {
            let was_known = self.users.contains_key(&owner);
            let history = self.user(owner);
            if history.created.is_none() {
                history.created = Some(created);
                history.observed_signup = !baseline && !was_known && !initial_bulk_import;
            }
        }
        for (owner, time) in changes.activity {
            let history = self.user(owner);
            if !initial_bulk_import
                && history
                    .created
                    .is_some_and(|created| time.saturating_sub(created) > WELCOME_DELAY)
            {
                history.activity.insert(time);
            }
        }
        for owner in changes.full.iter().copied() {
            let state = self.account_state(owner);
            self.record(owner, seq, state);
        }
        for owner in changes.scalar.difference(&changes.full) {
            let state = self
                .users
                .get(owner)
                .and_then(|index| self.history.accounts[*index].states.last())
                .map(|(_, row)| row.clone());
            if let Some(mut state) = state {
                state.egress = self.egress(*owner);
                self.record(*owner, seq, state);
            }
        }
        if changes.server_egress {
            let bytes = self
                .db
                .server_egress
                .as_ref()
                .map_or(0, |total| total.all_bandwidth() as u64);
            if self
                .history
                .server_egress
                .last()
                .is_none_or(|(_, prior)| *prior != bytes)
            {
                self.history.server_egress.push((seq, bytes));
            }
        }
        if let Some(time) = changes.time_hint {
            self.history.anchor(time, seq);
        }
        if self
            .history
            .checkpoints
            .last()
            .is_none_or(|(_, previous)| *previous != self.checkpoint)
        {
            self.history.checkpoints.push((seq, self.checkpoint));
        }
        self.history.latest = seq;
        Ok(())
    }

    fn egress(&self, owner: Owner) -> u64 {
        self.db
            .egress_by_owner
            .get(&owner)
            .map_or(0, |total| total.all_bandwidth() as u64)
    }

    fn account_state(&mut self, owner: Owner) -> AccountState {
        let mut state = AccountState { egress: self.egress(owner), ..Default::default() };
        let Some(account) = self.db.accounts.get(&owner) else { return state };
        if account.billing_info.is_premium() {
            state.provider = account
                .billing_info
                .billing_platform
                .as_ref()
                .map(|provider| match provider {
                    BillingPlatform::Stripe(_) => Provider::Stripe,
                    BillingPlatform::GooglePlay(_) => Provider::Play,
                    BillingPlatform::AppStore(_) => Provider::AppStore,
                });
        }
        let Some(owned) = self.db.owned_files.get(&owner) else { return state };
        state.files = owned.len();
        state.live = owned
            .iter()
            .any(|id| self.db.metas.get(id).is_some_and(FileLike::is_root));
        if !state.live {
            return state;
        }
        if let Ok(tree) = ServerTree::new(
            owner,
            &mut self.db.owned_files,
            &mut self.db.shared_files,
            &mut self.db.file_children,
            &mut self.db.metas,
        ) {
            let mut tree = tree.to_lazy();
            state.shared = tree.all_files().is_ok_and(|files| {
                files
                    .iter()
                    .any(|file| file.owner() != owner || file.is_shared())
            });
            state.storage = tree.calculate_usage(owner).ok();
        }
        state
    }

    fn record(&mut self, owner: Owner, seq: u64, state: AccountState) {
        let username = self
            .db
            .accounts
            .get(&owner)
            .filter(|_| state.live)
            .map(|account| account.username.clone());
        self.user(owner).username = username;
        let previous = self.user(owner).states.last().map(|(_, row)| row.clone());
        if previous.as_ref() == Some(&state) {
            return;
        }
        if let Some(previous) = previous {
            self.checkpoint.change(&previous, false);
        }
        self.checkpoint.change(&state, true);
        self.user(owner).states.push((seq, state));
    }
}

pub fn invalid(message: impl Into<String>) -> Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into()).into()
}
