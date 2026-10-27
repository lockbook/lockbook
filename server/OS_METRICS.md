# OS cohorts

`ServerV6.user_os` is an analytics-only `DbHashMapSet<Owner, OS>` containing
every known OS observed for an account. The router records normalized headers
only after signature verification, for existing, non-deleted accounts. Signup
records OS with the new account. Repeated observations don't write new events.
Missing/unrecognized headers don't add memberships; accounts without any known
OS are reported in `unknown` until a known OS is observed.

The map is appended to the schema; existing table IDs and account encoding stay
unchanged. Existing V6 databases open with an empty OS map. Once the new table
has been written, older server binaries cannot read its events: rollback needs
a compatible binary or a database restored from before rollout.

## Exported metrics

Existing metrics remain unchanged. New aggregate gauges are:

- `lockbook_metrics_by_os{type,os}`: the eight existing account statistic types
  plus `active_users_1h` (the same window as `lockbook_activity_by_user`).
- `lockbook_premium_users_by_os_and_payment_platform{platform,os}`.

The six OS values are `windows`, `linux`, `macOS`, `iOS`, `android`, `unknown`.
This adds 72 scalar series per server, independent of user count. No per-user OS
labels or OS combinations are exported. The existing Ops Agent scrape collects
these automatically; dashboards must select the new metrics and group by `os`.

Each account contributes its full metric value to every OS cohort it belongs to.
Do not sum OS cohorts for a unique-user or unique-storage total. Use existing
overall metrics instead. An active Android/iOS account counts in both buckets
even if its recent activity happened only on Android. Activity definitions stay
unchanged: successful metadata upserts, a 48-hour or 1-hour window, and the
existing five-second account-creation exclusion.

OS egress is the sum of existing lifetime per-account egress for non-deleted
accounts in each cohort, not transfer-time attribution. Its sum can differ from
the existing server-wide lifetime egress (which also includes deleted accounts).
Existing bandwidth accounting only records egress when bandwidth controls are
enabled. Documents/storage retain the existing metrics' counting semantics.

Normal account deletion retains memberships alongside the retained account and
username so `deleted_users` can be grouped by OS after restarts. Authenticated
requests after deletion cannot change that set. Admin disappearance removes the
memberships along with the account/username and excludes it from these counts,
matching existing metrics behavior.

## Existing accounts and historical backfill

Existing accounts gain memberships automatically on subsequent authenticated
requests. No OS history is inferred from payment platform or debug uploads.
Until requests or a historical import supply known OSes, they remain `unknown`.

To recover historical memberships, export trusted log observations containing
the **verified public key** and the OS from that same request. Exclude failed
signature checks, pre-signature request attempts, and pre-signup attempts.
Use the base64 compressed public key logged by `verified_request_signature`;
do not join by username or IP. The import format is JSONL:

```json
{"public_key":"<base64 compressed public key>","os":"android"}
{"public_key":"<same public key>","os":"iOS"}
```

Supply every available observed OS, rather than only the latest, to recover
historical sets. A latest-only export seeds a partial set that grows with future
requests. Missing log coverage cannot be reconstructed from the old database.

Deploy the compatible server binary before applying a backfill. Take a normal
database backup, then run against the existing `ServerV6` directory:

```sh
cargo run -p lockbook-server --bin backfill_user_os -- /path/to/ServerV6 observations.jsonl
cargo run -p lockbook-server --bin backfill_user_os -- /path/to/ServerV6 observations.jsonl --apply
```

Default mode reports additions without modifying memberships. `--apply` unions
observations with existing sets, including normally deleted accounts. Unknown
accounts are skipped. Input is fully validated before any membership changes.
The import is idempotent and uses the database's normal transaction locking.
