import Foundation
import SwiftUI
import BackgroundTasks

let macOSLogoutWindowSize = 1024.0
let macOSLogoutH1 = 24.0
let macOSLogoutH2 = 20.0
let macOSButtonWidth = 512.0
let xyOffset = 0.0

class WindowManager: NSObject, NSWindowDelegate {
    static let shared = WindowManager()
    private var windowRef: NSWindow?

    func openLogoutConfirmationWindow() {
        let contentView = NSHostingView(rootView: LogoutConfirmationView(
            h1: macOSLogoutH1,
            h2: macOSLogoutH2,
            buttonWidth: macOSButtonWidth))
        // Check if the window already exists and bring it to the front
        if let curWindow = windowRef {
            // replaces the contentView so that the @State variables reset
            curWindow.contentView = contentView
        } else {
            // Create a new window if it does not exist
            let window = NSWindow(
                contentRect: NSRect(x: xyOffset, y: xyOffset, width: macOSLogoutWindowSize, height: macOSLogoutWindowSize),
                styleMask: [.titled, .closable, .fullSizeContentView],
                backing: .buffered, defer: false)
            window.center()
            window.title = "Logout Confirmation"
            window.contentView = contentView
            window.isReleasedWhenClosed = false // Prevents the window from being deallocated when closed
            window.delegate = self
            
            windowRef = window
        }
        if let window = windowRef {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
        }
    }

    func windowWillClose(_ notification: Notification) {
        
    }
}
