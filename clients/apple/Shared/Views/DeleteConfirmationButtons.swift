import SwiftUI
import Foundation
import SwiftWorkspace

struct DeleteConfirmationButtons: View {
    
    var metas: [File]
    
    var body: some View {
        Group {
            Button("Delete \(metas.count == 1 ? "\"\(metas[0].name)\"" : "\(metas.count) files").", role: .destructive) {
                DI.files.deleteFiles(ids: metas.map({ $0.id }))
            }
            
            Button("Cancel", role: .cancel) {}
        }
    }
}
