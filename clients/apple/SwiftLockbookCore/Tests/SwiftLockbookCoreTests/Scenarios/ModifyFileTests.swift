import Foundation
@testable import SwiftLockbookCore

class ModifyFileTests: SLCTest {
    var account: Account?
    var root: FileMetadata?
    
    override func setUpWithError() throws {
        try super.setUpWithError()
        let _ = try core.api.createAccount(username: randomUsername(), apiLocation: systemApiLocation()).get()
        account = try core.api.getAccount().get()
        root = try core.api.getRoot().get()
    }

    func testUpdateContent1KB() throws {
        let resultCreateFile = core.api.createFile(name: randomFilename(), dirId: root!.id, isFolder: false)
        assertSuccess(resultCreateFile)
        let createdFile = try resultCreateFile.get()
        
        /// 1 KB of data
        let data = Data(count: 1000)
        measure {
            assertSuccess(core.api.updateFile(id: createdFile.id, content: data.base64EncodedString()))
            assertSuccess(core.api.synchronize())
            assertSuccess(core.api.getFile(id: createdFile.id)) { $0.secret == data.base64EncodedString() }
        }
    }
    
    func testUpdateContent1MB() throws {
        let resultCreateFile = core.api.createFile(name: randomFilename(), dirId: root!.id, isFolder: false)
        assertSuccess(resultCreateFile)
        let createdFile = try resultCreateFile.get()
        
        /// 1 MB of data
        let data = Data(count: 1000*1000)
        assertSuccess(core.api.updateFile(id: createdFile.id, content: data.base64EncodedString()))
        assertSuccess(core.api.synchronize())
        assertSuccess(core.api.getFile(id: createdFile.id)) { $0.secret == data.base64EncodedString() }
    }
    func testUpdateContent10MB() throws {
        let resultCreateFile = core.api.createFile(name: randomFilename(), dirId: root!.id, isFolder: false)
        assertSuccess(resultCreateFile)
        let createdFile = try resultCreateFile.get()
        
        /// 10 MB of data
        let data = Data(count: 10*1000*1000)
        assertSuccess(core.api.updateFile(id: createdFile.id, content: data.base64EncodedString()))
        assertSuccess(core.api.synchronize())
        assertSuccess(core.api.getFile(id: createdFile.id)) { $0.secret == data.base64EncodedString() }
    }
    
    func testRename() throws {
        let filename1 = randomFilename()
        
        let resultCreateFile = core.api.createFile(name: filename1, dirId: root!.id, isFolder: false)
        
        assertSuccess(resultCreateFile) { $0.name == filename1 }
        
        assertSuccess(core.api.listFiles()) { $0.allSatisfy { $0.name == filename1 || $0.id == root?.id } }
        
        let createdFile = try resultCreateFile.get()
        
        let filename2 = randomFilename()
        
        assertSuccess(core.api.renameFile(id: createdFile.id, name: filename2))
        
        assertSuccess(core.api.listFiles()) { $0.allSatisfy { $0.name == filename2 || $0.id == root?.id } }
    }
}
