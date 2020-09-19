//
//  LockbookApp.swift
//  Shared
//
//  Created by Raayan Pillai on 9/19/20.
//

import SwiftUI
import SwiftLockbookCore

@main
struct LockbookApp: App {
    let api = CoreApi(documentsDirectory: FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.path)
    
    var body: some Scene {
        api.initializeLogger()
        return WindowGroup {
            ContentView()
        }
    }
}
