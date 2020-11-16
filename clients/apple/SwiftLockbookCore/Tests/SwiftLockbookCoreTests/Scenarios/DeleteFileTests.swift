import XCTest
@testable import SwiftLockbookCore

class DeleteFileTests: SLCTest {
    var account: Account?
    var root: FileMetadata?

    override func setUpWithError() throws {
        try super.setUpWithError()
        let _ = try core.api.createAccount(username: randomUsername(), apiLocation: systemApiLocation()).get()
        account = try core.api.getAccount().get()
        root = try core.api.getRoot().get()
    }

    func testSimple() throws {
        let newFile = try core.api.createFile(name: randomFilename(), dirId: root!.id, isFolder: false).get()

        let resultDelete = core.api.deleteFile(id: newFile.id)

        assertSuccess(resultDelete)
    }

    func testMissingFile() throws {
        let resultDelete = core.api.deleteFile(id: UUID())

        assertFailure(resultDelete) { $0 == .init(.FileDoesNotExist) }
    }
}
