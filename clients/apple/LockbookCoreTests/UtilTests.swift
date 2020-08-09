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
}
