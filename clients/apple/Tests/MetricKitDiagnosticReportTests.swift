import Foundation
import MetricKit
import XCTest
@testable import LockbookDiagnostics

final class MetricKitDiagnosticReportTests: XCTestCase {
    private var directory: URL!

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    }

    override func tearDownWithError() throws {
        if FileManager.default.fileExists(atPath: directory.path) {
            try FileManager.default.removeItem(at: directory)
        }
    }

    @available(iOS 27.0, macOS 27.0, *)
    private func report(isCrash: Bool = true) throws -> DiagnosticReport {
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
          "timeRange": {"begin": 721692800, "end": 721692900},
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
        guard #available(iOS 27.0, macOS 27.0, *) else { throw XCTSkip("Requires the OS 27 MetricKit API") }
        try MetricKitCrashStore(directory: { self.directory }).save(report())
        let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
        let text = try String(contentsOf: XCTUnwrap(files.first), encoding: .utf8)
        XCTAssertTrue(text.hasPrefix("INFO: MetricKit crash in app.lockbook.crashed-build (version 26.10.22)\n"))
        let json = try XCTUnwrap(text.split(separator: "\n", maxSplits: 1).last)
        let envelope = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any])
        XCTAssertEqual(envelope["timeStampBegin"] as? String, "2023-11-14T22:13:20.000Z")
        XCTAssertEqual(envelope["timeStampEnd"] as? String, "2023-11-14T22:15:00.000Z")
        let data = try JSONSerialization.data(withJSONObject: XCTUnwrap(envelope["diagnostic"]))
        let saved = try JSONDecoder().decode(DiagnosticReport.self, from: data)
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
        guard #available(iOS 27.0, macOS 27.0, *) else { throw XCTSkip("Requires the OS 27 MetricKit API") }
        let store = MetricKitCrashStore(directory: { self.directory })
        // Decode separately to exercise encoding a fresh binary-info dictionary.
        for _ in 0..<20 {
            try store.save(report())
        }
        let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
        XCTAssertEqual(files.count, 1)
    }

    func testIgnoresTypedNonCrashReports() throws {
        guard #available(iOS 27.0, macOS 27.0, *) else { throw XCTSkip("Requires the OS 27 MetricKit API") }
        try MetricKitCrashStore(directory: {
            XCTFail("A hang must not resolve the crash-report directory")
            return self.directory
        }).save(report(isCrash: false))
        XCTAssertFalse(FileManager.default.fileExists(atPath: directory.path))
    }
}
