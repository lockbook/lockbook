import SwiftUI

struct ExitSheetButton: View {
    @Environment(\.dismiss) var dismiss
    
    var body: some View {
        Button(action: {
            dismiss()
        }, label: {
            Label("Exit", systemImage: "xmark")
                .labelStyle(.iconOnly)
        })
        .buttonStyle(.bordered)
        .clipShape(Circle())
    }
}
