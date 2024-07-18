import SwiftUI
import Foundation
import SwiftLockbookCore

struct DeleteConfirmationButtons: View {
    
    var meta: File
    
    var body: some View {
        Group {
            Button("Delete \"\(meta.name)\"", role: .destructive) {
                DI.files.deleteFile(id: meta.id)
            }
            
            Button("Cancel", role: .cancel) {}
        }
    }
}
