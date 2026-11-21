import SwiftUI
import SwiftWorkspace

struct SubAccountsView: View {
    let settingsModel: SettingsModel

    @State private var model = SubAccountsModel.shared
    @State private var showNew = false
    @State private var newLabel = ""
    @State private var error: String? = nil

    private var parent: String {
        AppState.shared.account?.username ?? ""
    }

    private var canCreate: Bool {
        settingsModel.isPremium == true && !model.working
    }

    var body: some View {
        Form {
            Section {
                ForEach(model.subAccounts) { subAccount in
                    NavigationLink {
                        SubAccountView(subAccount: subAccount)
                    } label: {
                        HStack {
                            Label(subAccount.username, systemImage: "person.badge.key")
                                .lineLimit(1)
                                .truncationMode(.middle)

                            Spacer()

                            Text(subAccount.usage.human)
                                .foregroundStyle(.secondary)
                        }
                    }
                }

                if !model.loaded {
                    if let loadError = model.loadError {
                        Text(loadError)
                            .foregroundStyle(.secondary)
                    } else {
                        ProgressView()
                            .frame(maxWidth: .infinity)
                    }
                } else if model.subAccounts.isEmpty {
                    Text("No sub-accounts")
                        .foregroundStyle(.secondary)
                }
            } footer: {
                Text("A sub-account has its own keys for a script, a tool or another person. It uses your storage and reaches only what you share with it.")
            }

            Section {
                #if os(iOS)
                    Button("New Sub-account", action: startNew)
                        .disabled(!canCreate)
                #else
                    HStack {
                        Text("New Sub-account")
                        Spacer()
                        Button("Create…", action: startNew)
                            .disabled(!canCreate)
                    }
                #endif

                if let error {
                    Text(error)
                        .foregroundStyle(.red)
                }
            } footer: {
                if settingsModel.isPremium == false {
                    Text("Creating a sub-account needs a premium subscription.")
                }
            }
        }
        .alert("New Sub-account", isPresented: $showNew) {
            TextField("Label", text: $newLabel)
                .autocapitalizationDisabled()
                .autocorrectionDisabled()

            Button("Create", action: create)

            Button("Cancel", role: .cancel) {}
        } message: {
            Text("It will be named \(parent)+label. A name can't be used again after its sub-account is deleted.")
        }
        #if os(iOS)
            .navigationTitle("Sub-accounts")
            .navigationBarTitleDisplayMode(.inline)
        #else
            .formStyle(.grouped)
            .toolbar {
                ToolbarItem(placement: .principal) {
                    Text("Sub-accounts").font(.headline)
                }
            }
        #endif
        .onAppear {
            model.load()

            if settingsModel.isPremium == nil {
                settingsModel.loadTier()
            }
        }
    }

    private func startNew() {
        newLabel = ""
        error = nil
        showNew = true
    }

    private func create() {
        let label = newLabel.trimmingCharacters(in: .whitespaces)
        guard !label.isEmpty else {
            return
        }

        model.create(label: label) { res in
            if case let .failure(err) = res {
                error = err.msg
            }
        }
    }
}

struct SubAccountView: View {
    let subAccount: SubAccount

    @Environment(\.dismiss) private var dismiss

    @State private var model = SubAccountsModel.shared
    @State private var keys: SubAccountKeys? = nil
    @State private var confirmDelete = false
    @State private var error: String? = nil

    var body: some View {
        Form {
            Section {
                LabeledContent("Username", value: subAccount.username)
                LabeledContent("Storage", value: subAccount.usage.human)
            }

            Section {
                #if os(iOS)
                    Button("Reveal Keys", action: revealKeys)
                #else
                    HStack {
                        Text("Keys")
                        Spacer()
                        Button("Reveal…", action: revealKeys)
                    }
                #endif
            } footer: {
                Text("Sign in as this sub-account with its keys. Anyone who has them can reach what you've shared with it.")
            }

            Section {
                #if os(iOS)
                    Button("Delete Sub-account", role: .destructive) {
                        confirmDelete = true
                    }
                #else
                    HStack {
                        Text("Delete Sub-account")
                        Spacer()
                        Button("Delete…", role: .destructive) {
                            confirmDelete = true
                        }
                    }
                #endif

                if let error {
                    Text(error)
                        .foregroundStyle(.red)
                }
            }
        }
        .disabled(model.working)
        .confirmationDialog(
            "Delete \(subAccount.username)?",
            isPresented: $confirmDelete,
            titleVisibility: .visible
        ) {
            Button("Delete", role: .destructive, action: delete)
        } message: {
            Text("The files it owns are deleted, and its name can't be used again.")
        }
        .navigationDestination(item: $keys) { keys in
            AccountKeysView(title: keys.username, accountKey: keys.key, phrase: keys.phrase)
                #if os(macOS)
                .toolbar {
                    ToolbarItem(placement: .principal) {
                        Text(keys.username).font(.headline)
                    }
                }
                #endif
        }
        #if os(iOS)
            .navigationTitle(subAccount.label)
            .navigationBarTitleDisplayMode(.inline)
        #else
            .formStyle(.grouped)
            .toolbar {
                ToolbarItem(placement: .principal) {
                    Text(subAccount.username).font(.headline)
                }
            }
        #endif
    }

    private func revealKeys() {
        error = nil

        AuthHelper.authenticateWithBiometricsOrPasscode { success in
            guard success else {
                return
            }

            model.exportKeys(username: subAccount.username) { res in
                switch res {
                case let .success(exported):
                    keys = exported
                case let .failure(err):
                    error = err.msg
                }
            }
        }
    }

    private func delete() {
        error = nil

        model.delete(username: subAccount.username) { res in
            switch res {
            case .success:
                dismiss()
            case let .failure(err):
                error = err.msg
            }
        }
    }
}

#Preview {
    NavigationStack {
        SubAccountsView(settingsModel: .preview)
    }
}
