# Apple crash reporting

The main app requires iOS/macOS 27 and starts MetricKit collection at startup.
One long-lived task consumes `MetricManager.diagnosticReports` and saves only
`DiagnosticResult.crash` events. The task retains its manager, writes reports serially
off the main actor, and is cancelled when the app's reporter is released. Performance
metrics, hangs, CPU exceptions, and disk-write diagnostics are not collected.

This covers system-reported native crashes, including Swift runtime traps, without
running Swift or Rust code in a signal handler. Apple controls diagnostic availability
and delivery, so registration does not guarantee a report for every termination.
App extensions have separate processes and are not subscribed by this change.

## Local files and uploads

Each diagnostic is atomically written to the same writable directory as lb-rs,
respecting `LOCKBOOK_PATH`, as `metrickit---<sha256>.log`. The file contains a readable
header followed by the complete Codable `DiagnosticReport`, including stack addresses,
binary UUIDs, bundle ID, application version/build, environment, and reporting interval.
Use the supplied interval rather than assuming an exact crash timestamp. Rust labels
the file modification time as its receipt time.

Sorted JSON makes repeated deliveries idempotent. The API delivers individual events;
different reporting intervals produce different IDs. There is no cross-source
deduplication: a fatal Rust panic may also appear in MetricKit.

lb-rs includes these files in the existing `DebugInfo.panics` array, so manual debug
info and the server's Discord attachment include them without a wire or server schema
change. Local collection applies to all users; automatic uploads retain the existing
client and server beta-user checks and only run after a successful sync. Uploads retain
the existing all-reports payload and local retention behavior. The server still sends
one notification when the number of stored reports increases, with the newest report
as its title and all reports attached.

The upload marker now fingerprints filenames, lengths, and modification times instead
of just the greatest panic timestamp. This detects delayed reports and batches with
equal receipt times without reading every report on each sync. It reuses the existing
`last_extracted_panic` database field: zero means no reports, negative values are
fingerprints, and a positive legacy timestamp causes one refresh on upgrade. The
marker is only saved after a successful upload. Collection failures are logged and
retried on a later sync rather than panicking.

Delayed MetricKit reports do not participate in the five-second `recent_panic` check
used by workspace restoration.

## Validation

Build with Xcode 27 or later. The app, bundled extensions, native workspace build,
and diagnostics test package target OS 27. Run the Swift tests on macOS 27 or later:

```sh
swift test --package-path clients/apple
cargo test -p lb-rs --lib service::debug::tests --locked
```

The Swift package compiles the production collector and tests crash filtering,
preservation of build and stack details, repeated deliveries, reporting intervals,
and write retries.
Rust tests cover discovery alongside legacy panic files, upload markers, and excluding
delayed reports from crash-loop recovery.

For a manual delivery check, launch a development app with an isolated `LOCKBOOK_PATH`
and use Xcode's **Debug > Simulate MetricKit Payloads**. Inspect the resulting
`metrickit---*.log` files and Settings > Debug Info. Simulated diagnostics are sample
data, not proof of actual crash delivery. To validate actual capture, trigger a Swift
trap in a disposable development build on a device, outside the debugger, then
relaunch and wait for MetricKit delivery. Use a test account/server to inspect uploads;
the collector itself never sends reports.

## Symbolication

Retain the release `.xcarchive` and matching dSYMs. Native addresses need symbols with
matching binary UUIDs to recover Swift function names and source lines. This change
preserves the information needed for symbolication but does not add a symbol store or
server-side symbolication. The version in each diagnostic can predate the currently
running app, so use the crashed build's symbols.

References: [MetricKit](https://developer.apple.com/documentation/metrickit),
[MetricManager](https://developer.apple.com/documentation/metrickit/metricmanager),
[DiagnosticReport](https://developer.apple.com/documentation/metrickit/diagnosticreport),
[matching dSYMs](https://developer.apple.com/documentation/xcode/locating-a-missing-debug-symbol-file).
