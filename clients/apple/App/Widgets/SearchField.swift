import SwiftUI

struct SearchField: View {
    let placeholder: String
    @Binding var text: String
    var focus: FocusState<Bool>.Binding? = nil
    var onSubmit: (() -> Void)? = nil

    @FocusState private var fallbackFocus: Bool

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .foregroundColor(.secondary)

            TextField(placeholder, text: $text)
                .textFieldStyle(.plain)
                .focused(focus ?? $fallbackFocus)
                .submitLabel(onSubmit == nil ? .return : .search)
                .onSubmit { onSubmit?() }
                .autocorrectionDisabled()
                .autocapitalizationDisabled()

            if !text.isEmpty {
                Button(action: { text = "" }) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(.secondary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(8)
        .background(RoundedRectangle(cornerRadius: 10).fill(Color.gray.opacity(0.15)))
        .padding(.horizontal)
        .padding(.vertical, 8)
    }
}
