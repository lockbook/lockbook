import Foundation
import SwiftUI
import SwiftLockbookCore
import BackgroundTasks

let macOSLogoutWindowSize = 1024.0
let macOSLogoutHeaderFontSize = 28.0
var macOSButtonWidth = 512.0

class WindowManager: NSObject, NSWindowDelegate {
    static let shared = WindowManager()
    private var windowRef: NSWindow?

    func openLogoutConfirmationWindow() {
        // Check if the window already exists and bring it to the front
        if let curWindow = windowRef {
            // replaces the contentView so that the @State variables reset
            curWindow.contentView = NSHostingView(rootView: LogoutConfirmationView(
                h1: macOSLogoutHeaderFontSize,
                h2: macOSLogoutHeaderFontSize-8,
                buttonWidth: macOSButtonWidth).environmentObject(DI.settings))
            curWindow.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }
        
        // Create a new window if it does not exist
        let window = NSWindow(
            contentRect: NSRect(x: 20, y: 20, width: macOSLogoutWindowSize, height: macOSLogoutWindowSize),
            styleMask: [.titled, .closable, .resizable, .fullSizeContentView],
            backing: .buffered, defer: false)
        window.center()
        window.title = "Logout Confirmation"
        window.contentView = NSHostingView(rootView: LogoutConfirmationView(
            h1: macOSLogoutHeaderFontSize,
            h2: macOSLogoutHeaderFontSize
            buttonWidth: macOSButtonWidth).environmentObject(DI.settings))
        window.isReleasedWhenClosed = false // Prevents the window from being deallocated when closed
        window.delegate = self
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        
        windowRef = window
    }

    func windowWillClose(_ notification: Notification) {
        
    }
}
