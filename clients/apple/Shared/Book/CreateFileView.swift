import SwiftUI
import SwiftLockbookCore

struct CreateFileView: View {
    @ObservedObject var core: Core
    @Binding var isPresented: Bool
    @State var newFileIsDoc: Bool = true
    @State var newFileName: String = ""
    let currentFolder: FileMetadataWithChildren

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
                            isPresented = false
                        case .failure(let err):
                            core.displayError(error: err)
                        }
                        
                    }
                }) {
                    Label("Create", systemImage: "plus.circle")
                        .foregroundColor(.green)
                }
                .keyboardShortcut(KeyEquivalent("j"), modifiers: .command)
                .padding(.horizontal, 10)
//                #if os(macOS)
//                Button("Dismiss", action: {
//                    isPresented = false
//                })
//                .keyboardShortcut(KeyEquivalent("f"), modifiers: .command)
//                #endif
            }
        }
    }
}

struct CreateFileView_Previews: PreviewProvider {
    static var previews: some View {
        CreateFileView(core: Core(), isPresented: .constant(true), currentFolder: FileMetadataWithChildren(meta: FakeApi().fileMetas[0], children: []))
    }
}
