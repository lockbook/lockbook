import SwiftUI
import SwiftLockbookCore

struct CreateFileView: View {
    @ObservedObject var core: Core
    @State var newFileIsDoc: Bool = true
    @State var newFileName: String = ""
    let currentFolder: FileMetadataWithChildren
    @Environment(\.presentationMode) var presentationMode
    
    var body: some View {
        VStack {
            TextField("\(currentFolder.meta.name)/", text: $newFileName)
                .disableAutocorrection(true)
                .multilineTextAlignment(.center)
            Toggle("Folder", isOn: $newFileIsDoc)
                .toggleStyle(FlipToggleStyle(left: ("Doc", "doc", .pink), right: ("Folder", "folder", .purple)))
                .padding(.vertical, 50)
                .keyboardShortcut(KeyEquivalent("d"), modifiers: .command)
            HStack {
                Button(action: {
                    if !newFileName.isEmpty {
                        switch core.api.createFile(name: newFileName, dirId: currentFolder.id, isFolder: !newFileIsDoc) {
                        case .success(_):
                            core.updateFiles()
                            presentationMode.wrappedValue.dismiss()
                        case .failure(let err):
                            core.handleError(err)
                        }
                        
                    }
                }) {
                    Label("Create", systemImage: "plus.circle")
                        .foregroundColor(.green)
                }
                .keyboardShortcut(KeyEquivalent("j"), modifiers: .command)
                .padding(.horizontal, 10)
            }
        }
    }
}

struct CreateFileView_Previews: PreviewProvider {
    static var previews: some View {
        CreateFileView(core: Core(), currentFolder: FileMetadataWithChildren(meta: FakeApi().fileMetas[0], children: []))
    }
}
