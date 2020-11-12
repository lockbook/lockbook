import SwiftUI
import SwiftLockbookCore

struct ActionsView: View {
    @ObservedObject var core: Core
    @State var newFileIsDoc: Bool = true
    @State var newFileName: String = ""
    let maybeSelected: FileMetadataWithChildren?
    @Environment(\.presentationMode) var presentationMode
    
    var body: some View {
        switch maybeSelected {
        case .some(let selected):
            return AnyView(VStack(spacing: 50) {
                TextField("\(selected.meta.name)/", text: $newFileName)
                    .disableAutocorrection(true)
                    .multilineTextAlignment(.center)
                Toggle("Folder", isOn: $newFileIsDoc)
                    .toggleStyle(FlipToggleStyle(left: ("Doc", "doc", .pink), right: ("Folder", "folder", .purple)))
                    .keyboardShortcut(KeyEquivalent("d"), modifiers: .command)
                Button(action: {
                    if !newFileName.isEmpty {
                        switch core.api.createFile(name: newFileName, dirId: selected.id, isFolder: !newFileIsDoc) {
                        case .success(_):
                            core.updateFiles()
                            presentationMode.wrappedValue.dismiss()
                        case .failure(let err):
                            core.handleError(err)
                        }
                    }
                }) {
                    Label("Create", systemImage: "plus")
                        .foregroundColor(.green)
                }
                Button(action: {
                    core.api.deleteFile(id: selected.id)
                }) {
                    Label("Delete", systemImage: "trash")
                        .foregroundColor(.red)
                }
            })
        case .none:
            return AnyView(VStack {
                Text("Selected a file first!")
            })
        }

    }
}

struct ActionsView_Previews: PreviewProvider {
    static var previews: some View {
        ActionsView(core: Core(), maybeSelected: FileMetadataWithChildren(meta: FakeApi().fileMetas[0], children: []))
    }
}
