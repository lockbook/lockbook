//
//  AppDelegate.swift
//  macos
//
//  Created by Raayan Pillai on 5/25/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Cocoa
import SwiftUI
import SwiftLockbookCore

@NSApplicationMain
class AppDelegate: NSObject, NSApplicationDelegate {

    var window: NSWindow!

    var documentsDirectory: String {
        return FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.appendingPathComponent(".lockbook").path
    }
    
    func applicationDidFinishLaunching(_ aNotification: Notification) {
        // Create the Lockbook Core Api with the path all our business happens
        let lockbookApi = CoreApi(documentsDirectory: documentsDirectory)
        // Initialize library logger
        lockbookApi.initializeLogger()
        // Create the SwiftUI view that provides the window contents.
        let contentView = ContentView(lockbookApi: lockbookApi)
        
        // Create the window and set the content view. 
        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 480, height: 300),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered, defer: false)
        window.center()
        window.setFrameAutosaveName("Main Window")
        window.contentView = NSHostingView(rootView: contentView)
        window.makeKeyAndOrderFront(nil)
    }

    func applicationWillTerminate(_ aNotification: Notification) {
        // Insert code here to tear down your application
    }


}

