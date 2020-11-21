import SwiftUI
import SwiftLockbookCore

struct ActionsView: View {
    @ObservedObject var core: Core
    let maybeSelected: FileMetadataWithChildren?
    @Environment(\.presentationMode) var presentationMode
    @Binding var creating: (FileMetadata, Bool)?

    var body: some View {
        switch maybeSelected {
        case .some(let selected):
            return AnyView(VStack(spacing: 50) {
                Button(action: {
                    self.creating = (selected.meta, false)
                    presentationMode.wrappedValue.dismiss()
                }) {
                    Label("File", systemImage: "plus")
                        .foregroundColor(.green)
                }
                Button(action: {
                    self.creating = (selected.meta, true)
                    presentationMode.wrappedValue.dismiss()
                }) {
                    Label("Folder", systemImage: "plus")
                        .foregroundColor(.green)
                }
                Button(action: {
                    switch core.api.deleteFile(id: selected.id) {
                    case .success(_):
                        core.updateFiles()
                        presentationMode.wrappedValue.dismiss()
                    case .failure(let err):
                        core.handleError(err)
                    }
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
        ActionsView(core: Core(), maybeSelected: FileMetadataWithChildren(meta: FakeApi().fileMetas[0], children: []), creating: .constant(.none))
    }
}
