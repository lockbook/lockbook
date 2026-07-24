import SwiftUI
import SwiftWorkspace

struct ShareFileSheet: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(WorkspaceInputState.self) private var workspaceInput

    let id: UUID

    @State private var username = ""
    @State private var mode: ShareMode = .write
    @State private var error = ""
    @State private var linkCopied = false
    @State private var exportedURL: URL? = nil
    @State private var copyResetTask: Task<Void, Never>? = nil
    @FocusState private var usernameFocused: Bool

    private var file: File? {
        filesModel.idsToFiles[id]
    }

    private var shares: [Share] {
        (file?.shares ?? []).sorted { lhs, rhs in
            if lhs.mode != rhs.mode {
                return lhs.mode == .write
            }

            return lhs.with < rhs.with
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Text("Share")
                    .font(.title2)
                    .bold()

                Spacer()

                Text(file?.name ?? "")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)

                #if os(macOS)
                    SheetCloseButton()
                #endif
            }

            HStack(spacing: 8) {
                TextField("Username", text: $username)
                    .textFieldStyle(.plain)
                    .autocapitalizationDisabled()
                    .focused($usernameFocused)
                    .onSubmit(addPerson)
                    .padding(10)
                    .background(RoundedRectangle(cornerRadius: 8).fill(Color.gray.opacity(0.15)))

                Picker("Access", selection: $mode) {
                    Text("Can edit").tag(ShareMode.write)
                    Text("Can view").tag(ShareMode.read)
                }
                .labelsHidden()
                .fixedSize()

                Button("Add", action: addPerson)
                    .buttonStyle(.borderedProminent)
                    .disabled(username.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }

            Text(error)
                .foregroundStyle(.red)
                .fontWeight(.bold)
                .lineLimit(1, reservesSpace: true)

            accessList

            Divider()

            HStack(spacing: 10) {
                Button(action: copyLink) {
                    Label(linkCopied ? "Copied" : "Copy link", systemImage: linkCopied ? "checkmark" : "link")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)

                shareExternally
            }
        }
        .padding()
        .onAppear {
            usernameFocused = true
            exportForSharing()
        }
        #if os(iOS)
            .presentationDetents([.medium, .large])
        #else
            .frame(width: 440)
        #endif
    }

    @ViewBuilder
    private var accessList: some View {
        if shares.isEmpty {
            Text("No one else has access.")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.vertical, 8)
        } else {
            ScrollView {
                VStack(spacing: 0) {
                    ForEach(shares, id: \.with) { share in
                        HStack {
                            Image(systemName: "person.crop.circle.fill")
                                .font(.title3)
                                .foregroundStyle(.secondary)

                            Text(share.with)

                            Spacer()

                            Text(share.mode == .write ? "Can edit" : "Can view")
                                .font(.callout)
                                .foregroundStyle(.secondary)
                        }
                        .padding(.vertical, 6)
                    }
                }
            }
            .frame(maxHeight: 220)
        }
    }

    @ViewBuilder
    private var shareExternally: some View {
        if let exportedURL {
            ShareLink(item: exportedURL) {
                Label("Share externally", systemImage: "square.and.arrow.up")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
        } else {
            Button {} label: {
                Label("Share externally", systemImage: "square.and.arrow.up")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(true)
        }
    }

    private func addPerson() {
        let name = username.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else {
            return
        }

        switch AppState.lb.shareFile(id: id, username: name, mode: mode) {
        case .success:
            username = ""
            error = ""
            workspaceInput.requestSync()
            filesModel.loadFiles()
        case let .failure(err):
            error = err.msg
        }
    }

    private func copyLink() {
        ClipboardHelper.copyFileLink(id)
        linkCopied = true

        copyResetTask?.cancel()
        copyResetTask = Task {
            try? await Task.sleep(for: .seconds(1.5))

            if !Task.isCancelled {
                linkCopied = false
            }
        }
    }

    private func exportForSharing() {
        guard let file else {
            return
        }

        let filesModel = filesModel

        DispatchQueue.global(qos: .userInitiated).async {
            guard let url = try? FileExporter.exportToTemp(file, edit: true, filesModel: filesModel) else {
                return
            }

            DispatchQueue.main.async {
                exportedURL = url
            }
        }
    }
}

#Preview {
    Color.clear
        .sheet(isPresented: .constant(true)) {
            ShareFileSheet(id: (AppState.lb as! MockLb).file1.id)
        }
        .environment(FilesModel.preview)
        .environment(WorkspaceInputState.preview)
}
