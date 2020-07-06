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
    var core = CoreApi(documentsDirectory: "/Users/raayanpillai/.lockbook")
    
    override class func setUp() {
        // Start logger
        init_logger()
    }
    
    func testCalculateWork() {
        let result = core.calculateWork()
        
        switch result {
            case .success(let workUnits):
                XCTAssertEqual(workUnits.count, 2)
            case .failure(let error):
                XCTFail(error.message)
        }
    }
    
    func testListFiles() {
        let files = core.listFiles(dirId: core.getRoot())
        
        XCTAssertEqual(files.count, 5)
    }
}

class UtilTests: XCTestCase {
    func testWorkUnitDecoding() {
        let bundle = Bundle(for: type(of: self))
        guard let url = bundle.url(forResource: "workUnits", withExtension: "json"), let data = try? String(contentsOf: url) else {
            return
        }
        
        if let workUnits: [WorkUnit] = (try? deserialize(jsonStr: data).get()) {
            XCTAssertEqual(workUnits.count, 3)
        } else {
            XCTFail()
        }
    }
}
