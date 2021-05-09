import XCTest
@testable import SwiftLockbookCore

class SyncFileTests: SLCTest {
    var account: Account?
    
    override func setUpWithError() throws {
        try super.setUpWithError()
        let _ = try core.api.createAccount(username: randomUsername(), apiLocation: systemApiLocation()).get()
        account = try core.api.getAccount().get()
    }
    
    func testBruteNoFiles() throws {
        let resultSync = core.api.syncAll()
        
        assertSuccess(resultSync)
    }
    
    func testBruteSomeFiles() throws {
        let root = try core.api.getRoot().get()
        
        let numberOfFiles = 5
        
        (0..<numberOfFiles).forEach { _ in assertSuccess(core.api.createFile(name: randomFilename(), dirId: root.id, isFolder: false)) }
        
        /// Verify all non-root files are unsynced
        assertSuccess(core.api.listFiles()) { $0.allSatisfy { $0.name == root.name || $0.metadataVersion == 0 } && $0.count == numberOfFiles+1 }
        
        let resultSync = core.api.syncAll()
        
        assertSuccess(resultSync)
        
        /// Verify all files are synced
        assertSuccess(core.api.listFiles()) { $0.allSatisfy { $0.metadataVersion > 0 } }
    }
    
    func testIterativeNoFiles() throws {
        let resultCalculate = core.api.calculateWork()
        
        assertSuccess(resultCalculate) { $0.workUnits.isEmpty }
    }
    
    func testLocalChangesNoFiles() throws {
        let resultCalculate = core.api.getLocalChanges()
        
        assertSuccess(resultCalculate) { $0.isEmpty }
    }
    
    func testBruteSomeFilesLocalChanges() throws {
        let root = try core.api.getRoot().get()
        
        let numberOfFiles = 5
        
        (0..<numberOfFiles).forEach { _ in assertSuccess(core.api.createFile(name: randomFilename(), dirId: root.id, isFolder: false)) }
        
        /// Verify all non-root files are unsynced
        assertSuccess(core.api.listFiles()) { $0.allSatisfy { $0.name == root.name || $0.metadataVersion == 0 } && $0.count == numberOfFiles+1 }
        
        var resultCalculate = core.api.getLocalChanges()

        assertSuccess(resultCalculate) { $0.count == 5 }
        
        let resultSync = core.api.syncAll()
        
        assertSuccess(resultSync)
        
        /// Verify all files are synced
        assertSuccess(core.api.listFiles()) { $0.allSatisfy { $0.metadataVersion > 0 } }
        
        resultCalculate = core.api.getLocalChanges()

        assertSuccess(resultCalculate) { $0.count == 0 }
    }
}
