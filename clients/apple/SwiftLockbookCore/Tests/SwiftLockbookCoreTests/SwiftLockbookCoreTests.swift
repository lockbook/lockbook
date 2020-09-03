import XCTest
@testable import SwiftLockbookCore

final class SwiftLockbookCoreTests: XCTestCase {
    static let fileMan = FileManager.init()
    static let tempDir = NSTemporaryDirectory().appending(UUID.init().uuidString)
    
    /// The following can be used to interface Swift code with a local lockbook instance! Useful for testing
    // CoreApi(documentsDirectory: "~/.lockbook")
    static let core = CoreApi(documentsDirectory: SwiftLockbookCoreTests.tempDir)
    
    override class func setUp() {
        super.setUp()
        
        print("Lockbook Dir", SwiftLockbookCoreTests.core.documentsDirectory)
    }
    
    override func setUp() {
        super.setUp()
        
        continueAfterFailure = false
        
        XCTAssertTrue(SwiftLockbookCoreTests.core.getApiLocation().contains("qa."))
    }
    
    func test00WhatEnvAmIUsing() {
        let apiLocation = SwiftLockbookCoreTests.core.getApiLocation()
        
        print(apiLocation)
        
        XCTAssertTrue(apiLocation.contains("qa."))
    }
    
    func test01CreateExportImportAccount() {
        let username = "swift"+UUID.init().uuidString.replacingOccurrences(of: "-", with: "")
        let result = SwiftLockbookCoreTests.core.createAccount(username: username)
        
        switch result {
        case .success(let acc):
            XCTAssertEqual(acc.username, username)
        case .failure(let err):
            XCTFail(err.message())
        }
        
        let exportResult = SwiftLockbookCoreTests.core.exportAccount()
        
        guard case .success(let accountString) = exportResult else {
            guard case .failure(let err) = exportResult else {
                return XCTFail()
            }
            return XCTFail(err.message())
        }
        
        try? SwiftLockbookCoreTests.fileMan.removeItem(atPath: SwiftLockbookCoreTests.tempDir)
        
        guard case .failure(_) = SwiftLockbookCoreTests.core.getAccount() else {
            return XCTFail("Account was found!")
        }
        
        let importResult = SwiftLockbookCoreTests.core.importAccount(accountString: accountString)
        
        switch importResult {
        case .success(let acc):
            XCTAssertEqual(acc.username, username)
        case .failure(let err):
            XCTFail(err.message())
        }
        
        let workResult = SwiftLockbookCoreTests.core.calculateWork()
        
        switch workResult {
        case .success(let work):
            XCTAssertEqual(work.workUnits.count, 1)
            
            let syncRes = SwiftLockbookCoreTests.core.synchronize()
            if case .failure(let err) = syncRes {
                return XCTFail(err.message())
            }
            
            switch  SwiftLockbookCoreTests.core.calculateWork() {
            case .success(let workMeta):
                XCTAssertEqual(workMeta.workUnits.count, 0)
            case .failure(let err):
                XCTFail(err.message())
            }
        case .failure(let err):
            XCTFail(err.message())
        }
    }
    
    func test02CreateFile() {
        let filename = "swiftfile"+UUID.init().uuidString.replacingOccurrences(of: "-", with: "")+".md"
        
        do {
            let root = try SwiftLockbookCoreTests.core.getRoot().get()
            
            let result = SwiftLockbookCoreTests.core.createFile(name: filename, dirId: root.id, isFolder: false)
            
            switch result {
            case .success(let file):
                XCTAssertEqual(file.name, filename)
            case .failure(let err):
                XCTFail(err.message())
            }
        } catch let err as ApplicationError {
           XCTFail(err.message())
       } catch {
           XCTFail(error.localizedDescription)
       }
    }
    
    func test03Sync() {
        let result = SwiftLockbookCoreTests.core.synchronize()
        
        switch result {
        case .success(_):
            return
        case .failure(let err):
            return XCTFail(err.message())
        }
    }
    
    func test04ListFiles() {
        do {
            let _ = try SwiftLockbookCoreTests.core.getRoot().get()
            let result = SwiftLockbookCoreTests.core.listFiles()
            
            switch result {
            case .success(let files):
                XCTAssertEqual(files.count, 2)
            case .failure(let err):
                XCTFail(err.message())
            }
        } catch let err as ApplicationError {
            XCTFail(err.message())
        } catch {
            XCTFail(error.localizedDescription)
        }
    }
    
    func test05CreateFile() {
        do {
            let root = try SwiftLockbookCoreTests.core.getRoot().get()
            
            let result = SwiftLockbookCoreTests.core.createFile(name: "test.md", dirId: root.id, isFolder: false)
            
            switch result {
            case .success(let meta):
                XCTAssertEqual(meta.name, "test.md")
            case .failure(let err):
                XCTFail(err.message())
            }
        } catch let err as ApplicationError {
           XCTFail(err.message())
       } catch {
           XCTFail(error.localizedDescription)
       }
    }
    
    func test06CalculateWork() {
        let result = SwiftLockbookCoreTests.core.calculateWork()
        
        switch result {
        case .success(let workMeta):
            XCTAssertEqual(workMeta.workUnits.count, 1)
        case .failure(let err):
            XCTFail(err.message())
        }
    }
    
    func test10FfiPerformance() {
        self.measureMetrics([XCTPerformanceMetric.wallClockTime], automaticallyStartMeasuring: false) {
            let apiLocation = SwiftLockbookCoreTests.core.getApiLocation()
            
            XCTAssertTrue(apiLocation.contains("qa"))
            
            let accountResult = SwiftLockbookCoreTests.core.getAccount()
            if case .failure(_) = accountResult {
                let newAccountResult = SwiftLockbookCoreTests.core.createAccount(username: "swiftperformance\(UUID.init().uuidString.prefix(5))")
                if case .failure(let err) = newAccountResult {
                    return XCTFail("Could not create account! \(err)")
                }
            }
            
            startMeasuring()
            let result = SwiftLockbookCoreTests.core.calculateWork()
            stopMeasuring()
            
            if case .failure(let err) = result {
                return XCTFail("Didn't calculate any work! \(err)")
            }
        }
    }


    static var allTests = [
        ("test00WhatEnvAmIUsing", test00WhatEnvAmIUsing),
        ("test01CreateExportImportAccount", test01CreateExportImportAccount),
        ("test02CreateFile", test02CreateFile),
        ("test03Sync", test03Sync),
        ("test04ListFiles", test04ListFiles),
        ("test05CreateFile", test05CreateFile),
        ("test06CalculateWork", test06CalculateWork),
        ("test10FfiPerformance", test10FfiPerformance)
    ]
}
