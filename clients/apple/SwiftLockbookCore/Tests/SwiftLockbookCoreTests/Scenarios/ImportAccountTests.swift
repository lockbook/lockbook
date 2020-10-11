import XCTest
@testable import SwiftLockbookCore

class ImportAccountTests: SLCTest {
    var known: (account: Account, accountString: String)?
    
    override func setUpWithError() throws {
        try super.setUpWithError()
        let account = try core.api.createAccount(username: randomUsername(), apiLocation: systemApiLocation()).get()
        let accountString = try core.api.exportAccount().get()
        known = (account, accountString)
        try core.cleanUp()
    }
    
    func testSimple() throws {
        let importResult = core.api.importAccount(accountString: known!.accountString)
        
        assertSuccess(importResult) { $0.username == known!.account.username }
    }
    
    func testBadAccountString() throws {
        let importResult = core.api.importAccount(accountString: "JUNK-ACCOUNT-STRING")
        
        assertFailure(importResult) { $0 == .Lockbook(.AccountStringCorrupted) }
    }
}
