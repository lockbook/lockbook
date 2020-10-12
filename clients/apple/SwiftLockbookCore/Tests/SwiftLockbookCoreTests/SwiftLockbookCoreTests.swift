import XCTest
@testable import SwiftLockbookCore


/// A suite to help manage the Swift liblockbook_core wrapper
struct CoreScenario {
    let dir: URL
    let api: CoreApi
    
    init() {
        dir = FileManager.default.temporaryDirectory.appendingPathComponent("SwiftLockbookCoreTests", isDirectory: false)
        api = CoreApi(documentsDirectory: dir.path)
        api.initializeLogger()
    }
    
    /// Creates a working directory for your testing, deletes if one already exists
    /// - Throws: A file-system error when it cannot perform an operation
    func setUp() throws {
        if (FileManager.default.fileExists(atPath: dir.path)) {
            try cleanUp()
        }
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: false, attributes: .none)
        
    }
    
    /// Deletes the testing directory
    /// - Throws: A file-system error if it can't delete
    func cleanUp() throws {
        try FileManager.default.removeItem(atPath: dir.path)
    }
}

/// SLCTest stands for SwiftLockbookCoreTest, this provides useful boiler plate for testing the Swift liblockbook_core wrapper
class SLCTest: XCTestCase {
    let core = CoreScenario()
    
    override func setUpWithError() throws {
        try super.setUpWithError()
        try core.setUp()
    }
    
    override func tearDownWithError() throws {
        try super.tearDownWithError()
        try core.cleanUp()
    }
}

/// An error relating to a SwiftLockbookCore test
enum SLCTestError: Error {
    case noApiLocation(String)
}

extension SLCTest {
    /// Retrieve the API location defined by an environment variable
    /// - Returns: The API if it is defined
    func systemApiLocation() throws -> String {
        let envVar = "API_URL"
        
        guard let location = ProcessInfo.processInfo.environment[envVar] else {
            throw SLCTestError.noApiLocation("You must define \(envVar)")
        }
        return location
    }
    
    /// Generates a random username
    /// - Returns: A random username
    func randomUsername() -> String {
        let validChars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
        return "SwiftIntegrationTest" + String((0..<10).compactMap { _ in validChars.randomElement() })
    }
    
    /// Generates a random filename
    /// - Returns: A random filename
    func randomFilename() -> String {
        let validChars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
        return String((0..<10).compactMap { _ in validChars.randomElement() })
    }
    
    /// Helper to verify that a result was successful and meets some criteria
    /// - Parameters:
    ///   - result: The result you want to verify
    ///   - validation: Some truth about the Result.success
    func assertSuccess<T, E: UiError>(_ result: FfiResult<T, E>, validation: (T) -> Bool = { _ in true }) {
        switch result {
        case .success(let t):
            XCTAssertTrue(validation(t), "Result validation failed!")
        case .failure(let error):
            XCTFail("Result was not a success! \(error)")
        }
    }
    
    /// Helper to verify that a result was a failure and the error meets some criteria
    /// - Parameters:
    ///   - result: The result you want to verify
    ///   - validation: Some truth about the Result.failure(ApplicationError)
    func assertFailure<T, E: UiError>(_ result: FfiResult<T, E>, validation: (FfiError<E>) -> Bool = { _ in true }) {
        switch result {
        case .success(let t):
            XCTFail("Result was not an error! \(t)")
        case .failure(let error):
            XCTAssertTrue(validation(error), "ApplicationError validation failed! \(error)")
        }
    }
    
    /// A helper to format test log messages
    /// - Parameter message: The thing you want to print
    func formatLog(_ message: String) -> String {
        "ℹ️\t\(message)"
    }
    
    func log(_ message: String) {
        print(formatLog(message))
    }
}
