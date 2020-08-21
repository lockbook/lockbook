//
//  UtilTests.swift
//  LockbookCoreTests
//
//  Created by Raayan Pillai on 8/9/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import XCTest
@testable import LockbookCore

class UtilTests: XCTestCase {
    func test01WorkUnitDecoding() {
        let bundle = Bundle(for: type(of: self))
        guard let url = bundle.url(forResource: "workUnits", withExtension: "json"), let data = try? Data(contentsOf: url) else {
            return XCTFail("Could not load JSON")
        }
        
        if let workUnits: [WorkUnit] = (try? deserialize(data: data).get()) {
            XCTAssertEqual(workUnits.count, 3)
        } else {
            XCTFail()
        }
    }
    
    func test02CalculateWorkDecode() {
        let bundle = Bundle(for: type(of: self))
        guard let url = bundle.url(forResource: "workResult", withExtension: "json"), let data = try? Data(contentsOf: url) else {
            return XCTFail("Could not load JSON")
        }
        
        let result: CoreResult<WorkMetadata> = deserializeResult(jsonResultStr: String(data: data, encoding: .utf8)!)
        
        switch result {
        case .success(let workMeta):
            XCTAssertEqual(workMeta.workUnits.count, 1)
        case .failure(let err):
            XCTFail(err.message())
        }
    }
}
