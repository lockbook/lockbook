import XCTest

#if !canImport(ObjectiveC)
public func allTests() -> [XCTestCaseEntry] {
    return [
        testCase(SwiftLockbookCoreTests.allTests),
        testCase(UtilTests.allTests),
    ]
}
#endif
