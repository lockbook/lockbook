import SwiftUI

class ClipboardHelper {
    static func copyToClipboard(_ text: String) {
        #if os(iOS)
        UIPasteboard.general.string = text
        #elseif os(macOS)
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
        #endif
    }
    
    static func copyFileLink(_ id: UUID) {
        ClipboardHelper.copyToClipboard("lb://\(id.uuidString.lowercased())")
    }
}
