# Apple crash reporting

The main iOS and macOS app starts MetricKit collection at startup. On OS 27 and later,
one long-lived task consumes `MetricManager.diagnosticReports` and saves only
`DiagnosticResult.crash` events. The task retains its manager, writes reports serially
off the main actor, and is cancelled when the app's reporter is released.

OS 26 uses a separate `MXMetricManagerSubscriber` compatibility adapter because the
replacement API requires OS 27. Only this adapter imports `pastDiagnosticPayloads`
to recover cached deliveries. The two collectors never run together. Remove the
deprecated adapter when the minimum OS reaches 27. Performance metrics, hangs, CPU
exceptions, and disk-write diagnostics are not collected by this integration.

This covers system-reported native crashes, including Swift runtime traps, without
running Swift or Rust code in a signal handler. Apple controls diagnostic availability
and delivery, so registration does not guarantee a report for every termination.
App extensions have separate processes and are not subscribed by this change.

## Local files and uploads

Each diagnostic is atomically written to the same writable directory as lb-rs,
respecting `LOCKBOOK_PATH`, as `metrickit---<sha256>.log`. The file contains a readable
header and JSON with the diagnostic (including stack addresses and binary UUIDs),
bundle ID, application version, and reporting interval. On OS 27, it preserves the
complete Codable `DiagnosticReport`, including the crashed build's environment and
build number; the compatibility adapter stores the legacy crash diagnostic JSON.
Use the supplied interval rather than assuming an exact crash timestamp. Rust labels
the file modification time as its receipt time.

Canonical JSON makes repeated deliveries idempotent. An occurrence number preserves
multiple identical diagnostics in a legacy payload; the modern API delivers individual
events. Different reporting intervals produce different IDs. There is no cross-source
or cross-API deduplication: a fatal Rust panic may also appear in MetricKit, and the
same crash delivered through both APIs across an OS upgrade may be stored twice.

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

Build with Xcode 27 or later for the new MetricKit SDK. Deployment targets remain
iOS/macOS 26. Run the standalone Swift tests on a Mac:

```sh
swift test --package-path clients/apple
cargo test -p lb-rs --lib service::debug::tests --locked
```

The Swift package compiles both production collectors and tests persistence, report
metadata, repeated deliveries, ordering, multiple occurrences, and write retries.
On macOS 27 and later, it also tests typed-report crash filtering, preservation of
build and stack details, and redelivery. Those tests are skipped on older systems.
Rust tests cover discovery alongside legacy panic files, upload markers, and excluding
delayed reports from crash-loop recovery.

For a manual delivery check, launch a development app with an isolated `LOCKBOOK_PATH`
and use Xcode's **Debug > Simulate MetricKit Payloads**. Inspect the resulting
`metrickit---*.log` files and Settings > Debug Info. Simulated diagnostics are sample
data, not proof of actual crash delivery. To validate actual capture, trigger a Swift
trap in a disposable development build on a device, outside the debugger, then
relaunch and wait for MetricKit delivery. Use a test account/server to inspect uploads;
the collectors themselves never send reports. Exercise both OS 26's compatibility
path and OS 27's async path when checking real delivery.

## Symbolication

Retain the release `.xcarchive` and matching dSYMs. Native addresses need symbols with
matching binary UUIDs to recover Swift function names and source lines. This change
preserves the information needed for symbolication but does not add a symbol store or
server-side symbolication. The version in each diagnostic can predate the currently
running app, so use the crashed build's symbols.

References: [MetricKit](https://developer.apple.com/documentation/metrickit),
[MetricManager](https://developer.apple.com/documentation/metrickit/metricmanager),
[DiagnosticReport](https://developer.apple.com/documentation/metrickit/diagnosticreport),
[MXCrashDiagnostic](https://developer.apple.com/documentation/metrickit/mxcrashdiagnostic),
[matching dSYMs](https://developer.apple.com/documentation/xcode/locating-a-missing-debug-symbol-file).
