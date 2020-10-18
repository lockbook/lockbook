import XCTest
@testable import SwiftLockbookCore

class CreateFileTests: SLCTest {
    var account: Account?
    var root: FileMetadata?
    
    override func setUpWithError() throws {
        try super.setUpWithError()
        let _ = try core.api.createAccount(username: randomUsername(), apiLocation: systemApiLocation()).get()
        account = try core.api.getAccount().get()
        root = try core.api.getRoot().get()
    }
    
    func testDeepFileCreation() throws {
        var lastFolder = root!
        let numFolders = 5
        
        for i in 0...numFolders {
            lastFolder = try core.api.createFile(name: randomFilename(), dirId: lastFolder.id, isFolder: i < numFolders).get()
        }
    }
}
