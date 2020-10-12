import XCTest
@testable import SwiftLockbookCore

class ExportAccountTests: SLCTest {
    func testSimple() throws {
        let username = randomUsername()
        
        let resultCreate = try core.api.createAccount(username: username, apiLocation: systemApiLocation())
        
        assertSuccess(resultCreate) { $0.username == username }

        let resultExport = core.api.exportAccount()
        
        assertSuccess(resultExport)
        
        try core.cleanUp()
        
        let resultGetAccount = core.api.getAccount()
        
//        assertFailure(resultGetAccount) { $0 == .Lockbook(.NoAccount) }
        
        
        let resultImport = try core.api.importAccount(accountString: resultExport.get())
        
        assertSuccess(resultImport)
    }
}
