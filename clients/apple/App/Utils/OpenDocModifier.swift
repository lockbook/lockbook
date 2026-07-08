import SwiftUI
import SwiftWorkspace

struct OpenDocModifier: ViewModifier {
    @Environment(\.colorScheme) private var colorScheme
    @Environment(WorkspaceOutputState.self) private var workspaceOutput

    let file: File

    func body(content: Content) -> some View {
        if workspaceOutput.openDoc == file.id {
            content
                .background(
                    RoundedRectangle(cornerRadius: 5, style: .continuous)
                        .foregroundStyle(Color.primary.opacity(colorScheme == .light ? 0.05 : 0.1))
                        .padding(.vertical, 2)
                )
        } else {
            content
        }
    }
}
