import XCTest
@testable import SwiftLockbookCore

class SyncTests: SLCTest {
    var account: Account?
    
    override func setUpWithError() throws {
        try super.setUpWithError()
        account = try core.api.createAccount(username: randomUsername(), apiLocation: systemApiLocation()).get()
    }
    
    func testBruteNoFiles() throws {
        let resultSync = core.api.synchronize()
        
        assertSuccess(resultSync)
    }
    
    func testBruteSomeFiles() throws {
        let root = try core.api.getRoot().get()
        
        let numberOfFiles = 5
        
        (0..<numberOfFiles).forEach { _ in assertSuccess(core.api.createFile(name: randomFilename(), dirId: root.id, isFolder: false)) }
        
        /// Verify all non-root files are unsynced
        assertSuccess(core.api.listFiles()) { $0.allSatisfy { $0.name == root.name || $0.metadataVersion == 0 } && $0.count == numberOfFiles+1 }
        
        let resultSync = core.api.synchronize()
        
        assertSuccess(resultSync)
        
        /// Verify all files are synced
        assertSuccess(core.api.listFiles()) { $0.allSatisfy { $0.metadataVersion > 0 } }
    }
    
    func testIterativeNoFiles() throws {
        let resultCalculate = core.api.calculateWork()
        
        assertSuccess(resultCalculate) { $0.workUnits.isEmpty }
    }
    
    func testIterativeSomeFiles() throws {
        let root = try core.api.getRoot().get()

        let resultCalculateEmpty = core.api.calculateWork()
        
        assertSuccess(resultCalculateEmpty) { $0.workUnits.isEmpty }
        
        let numberOfFiles = 5
        
        (0..<numberOfFiles).forEach { _ in assertSuccess(core.api.createFile(name: randomFilename(), dirId: root.id, isFolder: false)) }
        
        assertSuccess(core.api.listFiles()) { $0.allSatisfy { $0.name == root.name || $0.metadataVersion == 0 } && $0.count == numberOfFiles+1 }
        
        let resultCalculateWithSome = core.api.calculateWork()
        
        /// Ensure there are X work units for X new files
        assertSuccess(resultCalculateWithSome) { $0.workUnits.count == numberOfFiles }
        
        let work = try resultCalculateWithSome.get()
        
        for unit in work.workUnits {
            log("Syncing: \(unit.get().name)")
            assertSuccess(core.api.executeWork(work: unit))
        }
        
        assertSuccess(core.api.calculateWork()) { $0.workUnits.isEmpty }
    }
}
