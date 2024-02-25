import Foundation
import SwiftUI
import SwiftLockbookCore
import BackgroundTasks

class WindowManager: NSObject, NSWindowDelegate {
    static let shared = WindowManager()
    private var windowRef: NSWindow?

    func openLogoutConfirmationWindow() {
        // Check if the window already exists and bring it to the front
        if let curWindow = windowRef {
            curWindow.contentView = NSHostingView(rootView: LogoutConfirmationView().environmentObject(DI.settings))
            curWindow.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }
        
        // Create a new window if it does not exist
        let window = NSWindow(
            contentRect: NSRect(x: 20, y: 20, width: 1024, height: 1024),
            styleMask: [.titled, .closable, .resizable, .fullSizeContentView],
            backing: .buffered, defer: false)
        window.center()
        window.title = "Logout Confirmation"
        window.contentView = NSHostingView(rootView: LogoutConfirmationView().environmentObject(DI.settings))
        window.isReleasedWhenClosed = false // Prevents the window from being deallocated when closed
        window.delegate = self
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        
        // Keep a reference to the window
        windowRef = window
    }

    func windowWillClose(_ notification: Notification) {
        
    }
}
