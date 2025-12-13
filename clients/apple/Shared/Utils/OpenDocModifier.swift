import SwiftUI
import SwiftWorkspace

struct OpenDocModifier: ViewModifier {
    @Environment(\.colorScheme) var colorScheme
    @EnvironmentObject var fileTreeModel: FileTreeViewModel
    
    let file: File
        
    func body(content: Content) -> some View {
        if fileTreeModel.openDoc == file.id {
            content
                .background(
                    RoundedRectangle(cornerRadius: 5, style: .continuous)
                        .foregroundStyle( Color.primary.opacity(colorScheme == .light ? 0.05 : 0.1))
                        .padding(.vertical, 2)
                )
        } else {
            content
        }
    }
}
