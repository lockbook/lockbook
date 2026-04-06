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
        switch AppState.lb.getAccount() {
        case .success(let account):
            ClipboardHelper.copyToClipboard("\(account.apiUrl)/\(id.uuidString)")
        case .failure(let err):
            AppState.shared.error = .custom(title: "Failed to copy file link", msg: err.msg)
        }
    }
}
