import SwiftUI
import SwiftWorkspace

struct PinnedDocsView: View {
    @Environment(\.horizontalSizeClass) var horizontalSizeClass

    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceInput: WorkspaceInputState

    @ObservedObject var model: PinnedDocsViewModel

    var body: some View {
        ScrollView(.horizontal) {
            HStack {
                ForEach(model.pinnedDocs ?? []) { info in
                    Button(action: {
                        DispatchQueue.main.async {
                            if homeState.isSidebarFloating {
                                homeState.sidebarState = .closed
                            }

                            workspaceInput.openFile(id: info.id)
                        }
                    }) {
                        PinnedDocCell(info: info, model: model)
                    }
                    .buttonStyle(.plain)
                }
            }
            .frame(height: 80)
            .padding(.horizontal)
        }
        .listRowBackground(Color.clear)
        .listRowInsets(EdgeInsets())
    }
}

struct PinnedDocCell: View {
    let info: PinnedDocInfo

    @Environment(\.colorScheme) var colorScheme

    @ObservedObject var model: PinnedDocsViewModel

    var body: some View {
        VStack(alignment: .leading) {
            Text(info.name)
                .foregroundColor(.primary)

            HStack {
                Text(info.parentName)
                    .font(.caption)
                    .foregroundColor(.accentColor)

                Spacer()

                Text(info.lastModified)
                    .font(.caption)
                    .foregroundColor(.gray)
            }
            .padding(.top, 1)
        }
        .contentShape(Rectangle())
        .frame(maxWidth: 200)
        .modifier(PinnedDocBackground())
        .contextMenu {
            Button {
                model.unpinDoc(id: info.id)
            } label: {
                Label("Unpin", systemImage: "pin.slash")
            }
        }
    }
}

struct PinnedDocBackground: ViewModifier {
    @Environment(\.colorScheme) var colorScheme

    func body(content: Content) -> some View {
        content
            .padding(12)
            .background(
                RoundedRectangle(cornerRadius: 10)
                    .fill(colorScheme == .light ? Color.accentColor.opacity(0.08) : Color.accentColor.opacity(0.19))
            )
    }
}

#Preview {
    PinnedDocsView(model: .preview)
        .withCommonPreviewEnvironment()
}
