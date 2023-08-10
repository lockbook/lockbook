import XCTest
@testable import SwiftLockbookCore

class FFITests: XCTestCase {
    func freshCore() -> CoreApi {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("SwiftLockbookCoreTests/\(UUID().uuidString)", isDirectory: false)
        return CoreApi(dir.path, logs: false)
    }
    
    func testSimple() throws {
        let core = freshCore()
        let result = core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false)
        let _ = try core.getAccount().get()

        assertSuccess(result)
    }
    
    func testNoNetwork() throws {
        let core = freshCore()
        let result = core.createAccount(username: randomName(), apiLocation: "ftp://localhost:6969", welcomeDoc: false)
        assertFailure(result) { $0 == .init(.CouldNotReachServer) }
    }
    
    func testDeepFileCreation() throws {
        let core = freshCore()
        let _ = try core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false).get()
        var lastFolder = try core.getRoot().get()
        let numFolders = 5
        
        for i in 0...numFolders {
            lastFolder = try core.createFile(name: randomName(), dirId: lastFolder.id, isFolder: i < numFolders).get()
        }
    }
    
    
    func testSimpleDelete() throws {
        let core = freshCore()
        let _ = try core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false).get()
        let root = try core.getRoot().get()
        
        let newFile = try core.createFile(name: randomName(), dirId: root.id, isFolder: false).get()
        let resultDelete = core.deleteFile(id: newFile.id)
        
        assertSuccess(resultDelete)
    }
    
    func testMissingFile() throws {
        let core = freshCore()
        let _ = try core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false).get()

        let resultDelete = core.deleteFile(id: UUID())
        assertFailure(resultDelete) { $0 == .init(.FileDoesNotExist) }
    }
    
    func testSimpleImportExport() throws {
        var core = freshCore()
        let _ = try core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false).get()
        
        let resultExport = core.exportAccount()
        assertSuccess(resultExport)
        
        core = freshCore()
        let resultGetAccount = core.getAccount()
        assertFailure(resultGetAccount) { $0 ==  .init(.NoAccount) }
        
        let resultImport = try core.importAccount(accountString: resultExport.get())
        assertSuccess(resultImport)
    }
    
    func testBadAccountString() throws {
        let core = freshCore()
        let importResult = core.importAccount(accountString: "JUNK-ACCOUNT-STRING")
        assertFailure(importResult) { $0 == .init(.AccountStringCorrupted) }
    }
    
    func testUpdateContent1KB() throws {
        let core = freshCore()
        let _ = try core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false).get()
        let root = try core.getRoot().get()

        let resultCreateFile = core.createFile(name: randomName(), dirId: root.id, isFolder: false)
        assertSuccess(resultCreateFile)
        let createdFile = try resultCreateFile.get()
        
        /// 1 KB of data
        let data = Data(count: 1000)
        assertSuccess(core.updateFile(id: createdFile.id, content: data.base64EncodedString()))
        assertSuccess(core.backgroundSync())
        assertSuccess(core.getFile(id: createdFile.id)) { $0 == data.base64EncodedString() }
    }
    
    func testUpdateContent1MB() throws {
        let core = freshCore()
        let _ = try core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false).get()
        let root = try core.getRoot().get()
        
        let resultCreateFile = core.createFile(name: randomName(), dirId: root.id, isFolder: false)
        assertSuccess(resultCreateFile)
        let createdFile = try resultCreateFile.get()
        
        /// 1 MB of data
        let data = Data(count: 1000*1000)
        assertSuccess(core.updateFile(id: createdFile.id, content: data.base64EncodedString()))
        assertSuccess(core.backgroundSync())
        assertSuccess(core.getFile(id: createdFile.id)) { $0 == data.base64EncodedString() }
    }
    func testUpdateContent10MB() throws {
        let core = freshCore()
        let _ = try core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false).get()
        let root = try core.getRoot().get()
        
        let resultCreateFile = core.createFile(name: randomName(), dirId: root.id, isFolder: false)
        assertSuccess(resultCreateFile)
        let createdFile = try resultCreateFile.get()
        
        /// 10 MB of data
        let data = Data(count: 10*1000*1000)
        assertSuccess(core.updateFile(id: createdFile.id, content: data.base64EncodedString()))
        assertSuccess(core.backgroundSync())
        assertSuccess(core.getFile(id: createdFile.id)) { $0 == data.base64EncodedString() }
    }
    
    func testRename() throws {
        let core = freshCore()
        let _ = try core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false).get()
        let root = try core.getRoot().get()
        
        let filename1 = randomName()
        let resultCreateFile = core.createFile(name: filename1, dirId: root.id, isFolder: false)
        
        assertSuccess(resultCreateFile) { $0.name == filename1 }
        assertSuccess(core.listFiles()) { $0.allSatisfy { $0.name == filename1 || $0.id == root.id } }
        
        let createdFile = try resultCreateFile.get()
        let filename2 = randomName()
        
        assertSuccess(core.renameFile(id: createdFile.id, name: filename2))
        assertSuccess(core.listFiles()) { $0.allSatisfy { $0.name == filename2 || $0.id == root.id } }
    }
    
    func testBruteNoFiles() throws {
        let core = freshCore()
        let _ = try core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false).get()
        
        let resultSync = core.backgroundSync()
        assertSuccess(resultSync)
    }
    
    func testBruteSomeFiles() throws {
        let core = freshCore()
        let _ = try core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false).get()
        let root = try core.getRoot().get()
        
        let numberOfFiles = 5
        
        (0..<numberOfFiles).forEach { _ in assertSuccess(core.createFile(name: randomName(), dirId: root.id, isFolder: false)) }
        
        let resultSync = core.backgroundSync()
        
        assertSuccess(resultSync)
        
        /// Verify all files are syncedsmail/android/fix-work-unit
        assertSuccess(core.listFiles()) { $0.allSatisfy { $0.lastModified > 0 } }
    }
    
    func testIterativeNoFiles() throws {
        let core = freshCore()
        let _ = try core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false).get()
        
        let resultCalculate = core.calculateWork()
        
        assertSuccess(resultCalculate) { $0.workUnits.isEmpty }
    }
    
    func testLocalChangesNoFiles() throws {
        let core = freshCore()
        let _ = try core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false).get()
        
        let resultCalculate = core.getLocalChanges()
        
        assertSuccess(resultCalculate) { $0.isEmpty }
    }
    
    func testBruteSomeFilesLocalChanges() throws {
        let core = freshCore()
        let _ = try core.createAccount(username: randomName(), apiLocation: url(), welcomeDoc: false).get()
        let root = try core.getRoot().get()
        
        let numberOfFiles = 5
        
        (0..<numberOfFiles).forEach { _ in assertSuccess(core.createFile(name: randomName(), dirId: root.id, isFolder: false)) }
        
        var resultCalculate = core.getLocalChanges()

        assertSuccess(resultCalculate) { $0.count == 5 }
        
        let resultSync = core.backgroundSync()
        
        assertSuccess(resultSync)
        
        /// Verify all files are synced
        assertSuccess(core.listFiles()) { $0.allSatisfy { $0.lastModified > 0 } }
        
        resultCalculate = core.getLocalChanges()

        assertSuccess(resultCalculate) { $0.count == 0 }
    }
}

func url() -> String {
    let envVar = "API_URL"
    return ProcessInfo.processInfo.environment[envVar]!
}

/// Generates a random filename
/// - Returns: A random filename
func randomName() -> String {
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
        XCTAssertTrue(validation(t), "Result validation failed! \(t)")
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
        XCTAssertTrue(validation(error), "ApplicationError validation failed! \(error) \(error.message)")
    }
}

