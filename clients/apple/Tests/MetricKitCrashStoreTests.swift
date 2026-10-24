import Foundation
import MetricKit
import XCTest
@testable import LockbookDiagnostics

final class MetricKitCrashStoreTests: XCTestCase {
    private var directory: URL!

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    }

    override func tearDownWithError() throws {
        if FileManager.default.fileExists(atPath: directory.path) {
            try FileManager.default.removeItem(at: directory)
        }
    }

    private func report(isCrash: Bool = true, begin: Double = 721_692_800) throws -> DiagnosticReport {
        // The SDK exposes Decodable reports without a public memberwise initializer.
        let json = #"""
        {
          "crashDiagnostic": {
            "signal": 5,
            "callStackTree": {
              "callStackPerThread": true,
              "callStackThreads": [{"threadAttributed": true, "rootFrames": [{
                "binaryUUID": "11111111-1111-1111-1111-111111111111",
                "address": 4294967338, "offsetIntoBinaryTextSegment": 42, "subFrames": []
              }]}],
              "binaryInfo": [
                {"uuid": "11111111-1111-1111-1111-111111111111", "name": "Lockbook"},
                {"uuid": "22222222-2222-2222-2222-222222222222", "name": "libswiftCore"}
              ]
            }
          },
          "timeRange": {"begin": \#(begin), "end": \#(begin + 100)},
          "environment": {
            "regionFormat": "US",
            "osVersion": {"platform": "macOS", "number": "27.0", "buildNumber": "26A434"},
            "deviceType": "Mac", "platformArchitecture": "arm64", "lowPowerModeEnabled": false,
            "isTestFlightApp": false, "applicationVersion": "26.10.22", "applicationBuildVersion": "123",
            "pid": 42, "bundleIdentifier": "app.lockbook.crashed-build", "signpostData": [], "states": []
          }
        }
        """#
        let fixture = isCrash ? json : json
            .replacingOccurrences(of: "crashDiagnostic", with: "hangDiagnostic")
            .replacingOccurrences(of: #""signal": 5"#, with: #""hangDuration": {"value": 100, "unit": "ms"}"#)
        return try JSONDecoder().decode(DiagnosticReport.self, from: Data(fixture.utf8))
    }

    func testPreservesTypedCrashReportAndCrashedBuild() throws {
        try MetricKitCrashStore(directory: { self.directory }).save(report())
        let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
        let file = try XCTUnwrap(files.first)
        XCTAssertNotNil(file.lastPathComponent.range(of: #"^metrickit---[a-f0-9]{64}\.log$"#, options: .regularExpression))
        let text = try String(contentsOf: file, encoding: .utf8)
        XCTAssertTrue(text.hasPrefix("INFO: MetricKit crash in app.lockbook.crashed-build (version 26.10.22)\n"))
        let json = try XCTUnwrap(text.split(separator: "\n", maxSplits: 1).last)
        let saved = try JSONDecoder().decode(DiagnosticReport.self, from: Data(json.utf8))
        XCTAssertEqual(saved.timeRange.start, Date(timeIntervalSince1970: 1_700_000_000))
        XCTAssertEqual(saved.timeRange.end, Date(timeIntervalSince1970: 1_700_000_100))
        XCTAssertEqual(saved.environment.applicationBuildVersion, "123")
        XCTAssertEqual(saved.environment.osVersion.number, "27.0")
        guard case .crash(let crash) = saved.result else { return XCTFail("Expected a crash") }
        XCTAssertEqual(crash.signal, 5)
        let frame = try XCTUnwrap(crash.callStackTree.callStackThreads.first?.rootFrames.first)
        XCTAssertEqual(frame.address, 4_294_967_338)
        XCTAssertEqual(frame.offsetIntoBinaryTextSegment, 42)
        XCTAssertEqual(frame.binaryName(from: crash.callStackTree), "Lockbook")
    }

    func testTypedReportRedeliveryDoesNotDuplicateFiles() throws {
        let store = MetricKitCrashStore(directory: { self.directory })
        try store.save(report())
        let original = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
        let file = try XCTUnwrap(original.first)
        let oldDate = Date(timeIntervalSince1970: 1)
        try FileManager.default.setAttributes([.modificationDate: oldDate], ofItemAtPath: file.path)
        // Decode separately to exercise encoding a fresh binary-info dictionary.
        for _ in 0..<20 {
            try store.save(report())
        }
        let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
        XCTAssertEqual(files, original)
        let attributes = try FileManager.default.attributesOfItem(atPath: file.path)
        XCTAssertEqual(attributes[.modificationDate] as? Date, oldDate)
    }

    func testIgnoresTypedNonCrashReports() throws {
        try MetricKitCrashStore(directory: {
            XCTFail("A hang must not resolve the crash-report directory")
            return self.directory
        }).save(report(isCrash: false))
        XCTAssertFalse(FileManager.default.fileExists(atPath: directory.path))
    }

    func testLaterReportingIntervalIsANewReport() throws {
        let store = MetricKitCrashStore(directory: { self.directory })
        try store.save(report())
        try store.save(report(begin: 721_692_900))
        let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
        XCTAssertEqual(files.count, 2)
    }

    func testWriteFailureCanBeRetried() throws {
        let store = MetricKitCrashStore(directory: { self.directory })
        try Data().write(to: directory)
        XCTAssertThrowsError(try store.save(report()))
        try FileManager.default.removeItem(at: directory)
        try store.save(report())
        let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
        XCTAssertEqual(files.count, 1)
    }
}
