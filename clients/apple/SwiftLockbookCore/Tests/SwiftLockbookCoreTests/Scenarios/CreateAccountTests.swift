import XCTest
@testable import SwiftLockbookCore

class CreateAccountTests: SLCTest {
    func testSimple() throws {
        let result = core.api.createAccount(username: randomUsername(), apiLocation: try systemApiLocation())
        
        assertSuccess(result)
    }
    
    func testNoNetwork() throws {
        let result = core.api.createAccount(username: randomUsername(), apiLocation: "ftp://localhost:6969")
        
        assertFailure(result) { $0 == .UiError(.CouldNotReachServer) }
    }
}
