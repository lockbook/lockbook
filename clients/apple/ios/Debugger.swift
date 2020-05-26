//
//  Debugger.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/19/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

final class Debugger: ObservableObject {
    var lockbookApi: LockbookApi
    
    init() {
        self.lockbookApi = FakeApi()
    }
    
    init(lockbookApi: LockbookApi) {
        self.lockbookApi = lockbookApi
    }
}
