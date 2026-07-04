import SwiftUI
import SwiftWorkspace

struct CreateFolderSheet: View {
    @Environment(\.dismiss) private var dismiss

    let onCreated: (File) -> Void

    @State private var model: CreateFolderModel
    @FocusState private var nameFocused: Bool

    init(parent: File, onCreated: @escaping (File) -> Void) {
        self.onCreated = onCreated
        _model = State(initialValue: CreateFolderModel(parentId: parent.id))
    }

    var body: some View {
        @Bindable var model = model

        VStack(alignment: .leading, spacing: 12) {
            Text("New Folder")
                .font(.title2)
                .bold()

            HStack {
                Text("Parent Folder:")
                    .font(.callout)

                Text(model.parentPath ?? "...")
                    .lineLimit(2)
                    .font(.system(.callout, design: .monospaced))
            }

            TextField("Folder name", text: $model.name)
                .textFieldStyle(.roundedBorder)
                .autocapitalizationDisabled()
                .focused($nameFocused)
                .onSubmit(createAndDismiss)

            Text(model.error)
                .foregroundStyle(.red)
                .fontWeight(.bold)
                .lineLimit(1, reservesSpace: true)

            Button(action: createAndDismiss) {
                Text("Create")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(model.name.isEmpty)
        }
        .padding()
        .onAppear {
            nameFocused = true
        }
        #if os(iOS)
            .presentationDetents([.height(240)])
        #else
            .frame(width: 420)
        #endif
    }

    private func createAndDismiss() {
        guard let folder = model.createFolder() else {
            return
        }

        onCreated(folder)
        dismiss()
    }
}

@Observable class CreateFolderModel {
    var name = ""
    var error = ""
    var parentPath: String? = nil

    let parentId: UUID

    init(parentId: UUID) {
        self.parentId = parentId

        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.getPathById(id: parentId)

            DispatchQueue.main.async {
                switch res {
                case let .success(path):
                    self.parentPath = path
                case let .failure(err):
                    AppState.shared.error = .lb(error: err)
                }
            }
        }
    }

    func createFolder() -> File? {
        guard !name.isEmpty else {
            return nil
        }

        switch AppState.lb.createFile(name: name, parent: parentId, fileType: .folder) {
        case let .success(folder):
            return folder
        case let .failure(err):
            error = err.msg
            return nil
        }
    }
}

#Preview {
    Color.clear
        .sheet(isPresented: .constant(true)) {
            CreateFolderSheet(parent: FilesModel.preview.root ?? (AppState.lb as! MockLb).file0) { _ in }
        }
}
