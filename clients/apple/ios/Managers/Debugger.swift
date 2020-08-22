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
    
    func createFiles(count: Int) -> Void {
        switch self.lockbookApi.getRoot() {
        case .success(let dir):
            for _ in 0..<5 {
                let _ = self.lockbookApi.createFile(name: String(UUID.init().uuidString.prefix(5)), dirId: dir.id, isFolder: false)
            }
            return
        case .failure(_):
            return
        }
    }
}
