import SwiftUI
import SwiftWorkspace

struct WorkspaceTabsList: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(WorkspaceInputState.self) private var workspaceInput
    @Environment(WorkspaceOutputState.self) private var workspaceOutput

    @State private var tabIds: [UUID] = []

    var body: some View {
        List {
            ForEach(tabIds, id: \.self) { id in
                tabRow(id)
            }
        }
        .onAppear(perform: refreshTabs)
        .onChange(of: workspaceOutput.tabCount) {
            refreshTabs()
        }
        .onChange(of: workspaceOutput.openDoc) {
            refreshTabs()
        }
        .safeAreaInset(edge: .bottom) {
            Button {
                workspaceInput.closeAllTabs()
            } label: {
                Text("Close All")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .padding()
        }
    }

    private func tabRow(_ id: UUID) -> some View {
        let isCurrent = id == workspaceOutput.openDoc
        let name = filesModel.idsToFiles[id]?.name ?? "unknown"

        return Button {
            workspaceInput.openFile(id: id)
        } label: {
            HStack(spacing: 8) {
                Image(systemName: FileIconHelper.docNameToSystemImageName(name: name))
                    .foregroundStyle(isCurrent ? Color.accentColor : Color.secondary)

                Text(name)
                    .lineLimit(1)
                    .truncationMode(.tail)

                Spacer(minLength: 0)

                Button {
                    workspaceInput.closeDoc(id: id)
                } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .listRowBackground(isCurrent ? Color.accentColor.opacity(0.15) : Color.clear)
    }

    private func refreshTabs() {
        tabIds = workspaceInput.getTabsIds()
    }
}

#Preview {
    WorkspaceTabsList()
        .environment(FilesModel.preview)
        .environment(WorkspaceInputState.preview)
        .environment(WorkspaceOutputState.preview)
}
