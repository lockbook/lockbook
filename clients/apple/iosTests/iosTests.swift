//
//  iosTests.swift
//  iosTests
//
//  Created by Raayan Pillai on 7/6/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import XCTest
@testable import Lockbook

class CoreApiTests: XCTestCase {
    static let fileMan = FileManager.init()
    static let tempDir = NSTemporaryDirectory().appending(UUID.init().uuidString)
    static let core = CoreApi(documentsDirectory: CoreApiTests.tempDir)
//    let core = CoreApi(documentsDirectory: "/Users/raayanpillai/.lockbook")
    
    override class func setUp() {
        // Start logger
        init_logger()
    }
    
    override func setUp() {
        print(CoreApiTests.self.core.documentsDirectory)
    }
    
    func test00ImportAccount() {
        let bundle = Bundle(for: type(of: self))
        guard let url = bundle.url(forResource: "accountString", withExtension: "txt"), let data = try? String(contentsOf: url) else {
            return XCTFail("Could not load Account String")
        }
        
        let result = CoreApiTests.core.importAccount(accountString: data)
        
        switch result {
        case .success(let account):
            XCTAssertEqual(account.username, "raayan")
        case .failure(let error):
            XCTFail(error.message)
        }
        
        try? CoreApiTests.fileMan.removeItem(atPath: CoreApiTests.tempDir)
    }
    
    func test01CreateAccount() {
        let username = "swift"+UUID.init().uuidString.replacingOccurrences(of: "-", with: "")
        let result = CoreApiTests.core.createAccount(username: username)
        
        switch result {
        case .success(let acc):
            XCTAssertEqual(acc.username, username)
        case .failure(let err):
            XCTFail(err.message)
        }
    }
    
    func test02CreateFile() {
        let filename = "swiftfile"+UUID.init().uuidString.replacingOccurrences(of: "-", with: "")+".md"
        
        guard let root = try? CoreApiTests.core.getRoot().get() else {
            return XCTFail("Could not get root!")
        }
        
        let result = CoreApiTests.core.createFile(name: filename, dirId: root.id, isFolder: false)
        
        switch result {
        case .success(let file):
            XCTAssertEqual(file.name, filename)
        case .failure(let err):
            XCTFail(err.message)
        }
    }
    
    func test02Sync() {
        let result = CoreApiTests.core.synchronize()
        
        switch result {
        case .success(let b):
            XCTAssert(b)
        case .failure(let error):
            XCTFail(error.message)
        }
    }
    
    func test03ListFiles() {
        do {
            let root = try CoreApiTests.core.getRoot().get()
            let result = CoreApiTests.core.listFiles(dirId: root.id)
            
            switch result {
            case .success(let files):
                XCTAssertEqual(files.count, 1)
            case .failure(let error):
                XCTFail(error.message)
            }
        } catch let error as CoreError {
            XCTFail(error.message)
        } catch {
            XCTFail(error.localizedDescription)
        }
    }
    
    func test04CreateFile() {
        guard let root = try? CoreApiTests.core.getRoot().get() else {
            return XCTFail("Couldn't get root!")
        }
        
        let result = CoreApiTests.core.createFile(name: "test.md", dirId: root.id, isFolder: false)
        
        switch result {
        case .success(let meta):
            XCTAssertEqual(meta.name, "test.md")
        case .failure(let error):
            XCTFail(error.message)
        }
    }
    
    func test05CalculateWork() {
        let result = CoreApiTests.core.calculateWork()
        
        switch result {
        case .success(let workUnits):
            XCTAssertEqual(workUnits.count, 1)
        case .failure(let error):
            XCTFail(error.message)
        }
    }
}

class UtilTests: XCTestCase {
    func testWorkUnitDecoding() {
        let bundle = Bundle(for: type(of: self))
        guard let url = bundle.url(forResource: "workUnits", withExtension: "json"), let data = try? String(contentsOf: url) else {
            return XCTFail("Could not load JSON")
        }
        
        if let workUnits: [WorkUnit] = (try? deserialize(jsonStr: data).get()) {
            XCTAssertEqual(workUnits.count, 3)
        } else {
            XCTFail()
        }
    }
}
