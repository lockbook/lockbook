//
//  LbSwiftTests.swift
//  LbSwiftTests
//
//  Created by Smail Barkouch on 10/27/24.
//

import Testing
import SwiftWorkspace

struct LbSwiftTests {

    @Test func example() async throws {
        // Write your test here and use APIs like `#expect(...)` to check expected conditions.
        let lb = Lb(writablePath: "/tmp/swifttests", logs: false)
        
        print("the res: \(lb.calculateWork())")
    }
}
