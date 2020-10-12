import XCTest
@testable import SwiftLockbookCore

class ImportAccountTests: SLCTest {
    var known: (account: Account, accountString: String)?
    
    override func setUpWithError() throws {
        try super.setUpWithError()
        let _ = try core.api.createAccount(username: randomUsername(), apiLocation: systemApiLocation()).get()
        let account = try core.api.getAccount().get()
        let accountString = try core.api.exportAccount().get()
        known = (account, accountString)
        try core.cleanUp()
    }
    
    func testSimple() throws {
        let importResult = core.api.importAccount(accountString: known!.accountString)
        
        assertSuccess(importResult)
        
        let getResult = core.api.getAccount()
        
        assertSuccess(getResult) { $0.username == known!.account.username }
    }
    
    func testBadAccountString() throws {
        let importResult = core.api.importAccount(accountString: "JUNK-ACCOUNT-STRING")
        
        assertFailure(importResult) { $0 == .init(.AccountStringCorrupted) }
    }
}
