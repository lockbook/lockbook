import Foundation
import XCTest
@testable import LockbookDiagnostics

final class MetricKitCrashStoreTests: XCTestCase {
    private var directory: URL!
    private let begin = Date(timeIntervalSince1970: 1_700_000_000)
    private let end = Date(timeIntervalSince1970: 1_700_000_100)

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    }

    override func tearDownWithError() throws {
        if FileManager.default.fileExists(atPath: directory.path) {
            try FileManager.default.removeItem(at: directory)
        }
    }

    private func diagnostic(_ json: String) -> MetricKitCrashStore.Diagnostic {
        .init(applicationVersion: "26.10.22", json: Data(json.utf8))
    }

    private func save(_ crashes: [MetricKitCrashStore.Diagnostic]) throws {
        try MetricKitCrashStore(directory: { self.directory }).save(
            crashes, begin: begin, end: end, bundleIdentifier: "app.lockbook"
        )
    }

    private func files() throws -> [URL] {
        try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
    }

    func testEmptyPayloadDoesNotResolveDataDirectory() throws {
        try MetricKitCrashStore(directory: {
            XCTFail("Non-crash payload must not initialize the data directory")
            return self.directory
        }).save([], begin: begin, end: end, bundleIdentifier: "app.lockbook")
        XCTAssertFalse(FileManager.default.fileExists(atPath: directory.path))
    }

    func testPreservesCrashContext() throws {
        try save([diagnostic(#"{"signal":5,"callStackTree":{"threads":[{"frames":[{"binaryUUID":"ABC","offsetIntoBinaryTextSegment":42}]}]}}"#)])
        let file = try XCTUnwrap(files().first)
        XCTAssertNotNil(file.lastPathComponent.range(of: #"^metrickit---[a-f0-9]{64}\.log$"#, options: .regularExpression))
        let text = try String(contentsOf: file, encoding: .utf8)
        XCTAssertTrue(text.hasPrefix("INFO: MetricKit crash in app.lockbook (version 26.10.22)\n"))
        let json = try XCTUnwrap(text.split(separator: "\n", maxSplits: 1).last)
        let report = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any])
        XCTAssertEqual(report["source"] as? String, "MetricKit")
        XCTAssertEqual(report["applicationVersion"] as? String, "26.10.22")
        XCTAssertEqual(report["timeStampBegin"] as? String, "2023-11-14T22:13:20.000Z")
        XCTAssertEqual(report["timeStampEnd"] as? String, "2023-11-14T22:15:00.000Z")
        let crash = try XCTUnwrap(report["diagnostic"] as? [String: Any])
        XCTAssertEqual(crash["signal"] as? Int, 5)
        XCTAssertNotNil(crash["callStackTree"])
    }

    func testRedeliveryAndReorderingDoNotRewriteReports() throws {
        let first = diagnostic(#"{"signal":5,"exceptionType":6}"#)
        let second = diagnostic(#"{"signal":11}"#)
        try save([first, first, second])
        let original = try files()
        XCTAssertEqual(original.count, 3, "Repeated identical crashes in one payload are separate occurrences")
        let oldDate = Date(timeIntervalSince1970: 1)
        for file in original {
            try FileManager.default.setAttributes([.modificationDate: oldDate], ofItemAtPath: file.path)
        }
        try save([second, diagnostic(#"{"exceptionType":6,"signal":5}"#), first])
        XCTAssertEqual(Set(try files()), Set(original))
        for file in original {
            let attributes = try FileManager.default.attributesOfItem(atPath: file.path)
            XCTAssertEqual(attributes[.modificationDate] as? Date, oldDate)
        }
    }

    func testLaterReportingIntervalIsANewReport() throws {
        let crash = diagnostic(#"{"signal":5}"#)
        try save([crash])
        try MetricKitCrashStore(directory: { self.directory }).save(
            [crash], begin: end, end: end.addingTimeInterval(100), bundleIdentifier: "app.lockbook"
        )
        XCTAssertEqual(try files().count, 2)
    }

    func testWriteFailureCanBeRetried() throws {
        try Data().write(to: directory)
        let crash = diagnostic(#"{"signal":5}"#)
        XCTAssertThrowsError(try save([crash]))
        try FileManager.default.removeItem(at: directory)
        try save([crash])
        XCTAssertEqual(try files().count, 1)
    }
}
