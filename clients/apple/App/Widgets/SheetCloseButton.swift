import SwiftUI

struct SheetCloseButton: View {
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        Button {
            dismiss()
        } label: {
            Image(systemName: "xmark.circle.fill")
                .font(.title3)
                .symbolRenderingMode(.hierarchical)
                .foregroundStyle(.secondary)
        }
        .buttonStyle(.plain)
        .keyboardShortcut(.cancelAction)
    }
}
