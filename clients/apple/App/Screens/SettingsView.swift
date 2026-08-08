import SwiftUI
import SwiftWorkspace

#if os(iOS)
    struct SettingsView: View {
        @State private var model = SettingsModel()

        @State private var showAccountKeys = false

        @AppStorage("contactLinkedSites") private var contactLinkedSites: Bool = false

        var body: some View {
            Form {
                Section("Account") {
                    UsernameRow()

                    Button("Reveal Account Keys") {
                        AuthHelper.authenticateWithBiometricsOrPasscode { success in
                            showAccountKeys = success
                        }
                    }

                    LogoutButton()
                }

                Section("Usage") {
                    UsageSettingsRows(model: model)
                }

                Section(
                    header: Text("Editor"),
                    footer: Text(
                        "Showing titles and preview cards means contacting the linked site, which reveals your IP address and that you opened the note. Off by default."
                    )
                ) {
                    Toggle("Fetch link previews", isOn: $contactLinkedSites)
                }

                Section("Privacy") {
                    Text("[Privacy Policy](https://lockbook.net/privacy-policy)")
                        .foregroundColor(.accentColor)

                    Text("[Terms of Service](https://lockbook.net/tos)")
                        .foregroundColor(.accentColor)

                    DeleteAccountButton(model: model)
                }

                Section("Debug") {
                    ServerRow()

                    NavigationLink("Debug Info") {
                        DebugView()
                    }
                }
            }
            .navigationDestination(isPresented: $showAccountKeys) {
                AccountKeysView()
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.large)
        }
    }

#else
    struct SettingsView: View {
        @State private var model = SettingsModel()

        var body: some View {
            NavigationStack {
                TabView {
                    SettingsAccountView(model: model)
                        .tabItem {
                            Label("Account", systemImage: "person")
                        }

                    SettingsUsageView(model: model)
                        .tabItem {
                            Label("Usage", systemImage: "externaldrive")
                        }

                    SettingsEditorView()
                        .tabItem {
                            Label("Editor", systemImage: "doc.text")
                        }

                    SettingsDebugView(model: model)
                        .tabItem {
                            Label("Debug", systemImage: "hammer")
                        }
                }
                .navigationTitle("Settings")
            }
            .frame(width: 500, height: 400)
        }
    }

    struct SettingsAccountView: View {
        let model: SettingsModel

        @State private var showAccountKeys = false

        var body: some View {
            Form {
                Section("Account") {
                    UsernameRow()

                    Button(action: {
                        AuthHelper.authenticateWithBiometricsOrPasscode { success in
                            showAccountKeys = success
                        }
                    }, label: {
                        Text("Reveal Account Keys")
                            .foregroundStyle(.foreground)
                        Spacer()
                        Image(systemName: "chevron.right")
                            .imageScale(.small)
                            .foregroundStyle(.gray)
                    })
                    .buttonStyle(.borderless)

                    HStack {
                        Text("Logout")
                        Spacer()
                        LogoutButton()
                    }
                }

                Section("Privacy") {
                    HStack {
                        Text("Privacy Policy")
                        Spacer()
                        Link("Open in browser", destination: URL(string: "https://lockbook.net/privacy-policy")!)
                    }

                    HStack {
                        Text("Terms of Service")
                        Spacer()
                        Link("Open in browser", destination: URL(string: "https://lockbook.net/tos")!)
                    }

                    HStack {
                        Text("Delete Account")
                        Spacer()
                        DeleteAccountButton(model: model)
                    }
                }
            }
            .formStyle(.grouped)
            .navigationDestination(isPresented: $showAccountKeys) {
                AccountKeysView()
                    .toolbar {
                        ToolbarItem(placement: .principal) {
                            Text("Account Keys").font(.headline)
                        }
                    }
            }
        }
    }

    struct SettingsUsageView: View {
        let model: SettingsModel

        var body: some View {
            Form {
                Section("Usage") {
                    UsageSettingsRows(model: model)
                }
            }
            .formStyle(.grouped)
        }
    }

    struct SettingsEditorView: View {
        @AppStorage("contactLinkedSites") private var contactLinkedSites: Bool = false

        var body: some View {
            Form {
                Section("Link Previews") {
                    Toggle("Fetch link previews", isOn: $contactLinkedSites)
                    Text(
                        "Showing titles and preview cards means contacting the linked site, which reveals your IP address and that you opened the note. Off by default."
                    )
                    .font(.caption)
                    .foregroundColor(.secondary)
                }
            }
            .formStyle(.grouped)
        }
    }

    struct SettingsDebugView: View {
        let model: SettingsModel

        var body: some View {
            Form {
                Section("Debug") {
                    ServerRow()

                    DebugView()
                }
            }
            .formStyle(.grouped)
        }
    }
#endif

struct UsernameRow: View {
    var body: some View {
        if let account = AppState.shared.account {
            HStack {
                Text("Username:")
                Spacer()
                Text(account.username)
            }
        } else {
            ProgressView()
        }
    }
}

struct ServerRow: View {
    var body: some View {
        if let account = AppState.shared.account {
            HStack {
                Text("Server:")
                    .padding(.trailing, 10)
                Text(account.apiUrl)
                    .lineLimit(1)
                    .truncationMode(.head)
                    .frame(maxWidth: .infinity, alignment: .trailing)
            }
        } else {
            ProgressView()
        }
    }
}

struct LogoutButton: View {
    @State private var confirmLogout = false

    var body: some View {
        Button("Logout", role: .destructive) {
            confirmLogout = true
        }
        .confirmationDialog(
            "Are you sure? Please make sure your key is backed up.",
            isPresented: $confirmLogout,
            titleVisibility: .visible
        ) {
            Button("Logout", role: .destructive) {
                AppState.lb.logoutAndExit()
            }
        }
    }
}

struct DeleteAccountButton: View {
    let model: SettingsModel

    @State private var confirmDeleteAccount = false

    var body: some View {
        Button("Delete Account", role: .destructive) {
            confirmDeleteAccount = true
        }
        .confirmationDialog(
            "Are you sure you want to delete your account?",
            isPresented: $confirmDeleteAccount,
            titleVisibility: .visible
        ) {
            Button("Delete account", role: .destructive) {
                model.deleteAccountAndExit()
            }
        }
    }
}

struct UsageSettingsRows: View {
    let model: SettingsModel

    @State private var confirmCancelSubscription = false
    #if os(macOS)
        @AppStorage("usageBarMode") private var usageBarMode: UsageBarDisplayMode = .whenHalf
    #endif

    var body: some View {
        if let isPremium = model.isPremium {
            HStack {
                Text("Current Tier:")
                Spacer()
                Text(isPremium ? "Premium" : "Free")
            }

            if !isPremium {
                NavigationLink("Upgrade now") {
                    UpgradeAccountView(settingsModel: model)
                }
            }
        }

        if let usage = model.usage {
            VStack {
                HStack {
                    Text("Server Utilization:")
                    Spacer()
                    Text("\(usage.serverUsedHuman) / \(usage.serverCapHuman)")
                }

                ProgressView(value: Double(usage.serverUsedExact), total: Double(usage.serverCapExact))
                    .padding(.top, 10)
                    .padding(.bottom, 8)
            }
        } else {
            ProgressView()
        }

        #if os(macOS)
            Picker("Sidebar usage bar", selection: $usageBarMode) {
                ForEach(UsageBarDisplayMode.allCases) { mode in
                    Text(mode.label).tag(mode)
                }
            }
        #endif

        if model.isPremium == true {
            Button("Cancel Subscription", role: .destructive) {
                confirmCancelSubscription = true
            }
            .confirmationDialog(
                "Are you sure you want to cancel your subscription?",
                isPresented: $confirmCancelSubscription,
                titleVisibility: .visible
            ) {
                Button("Confirm", role: .destructive) {
                    model.cancelSubscription()
                }
            }
        }
    }
}

struct AccountKeysView: View {
    let accountKey = (try? AppState.lb.exportAccountPrivateKey().get()) ?? "ERROR"

    var body: some View {
        Form {
            Section("Phrase") {
                AccountPhraseView(includeBackground: false)
            }

            Section("Compact") {
                VStack {
                    HStack {
                        Text(accountKey)
                            .font(.system(.body, design: .monospaced))
                            .lineLimit(1)
                            .truncationMode(.middle)
                            .padding(10)

                        Spacer()

                        Button(action: {
                            ClipboardHelper.copyToClipboard(accountKey)
                        }) {
                            Image(systemName: "doc.on.doc")
                                .foregroundColor(.accentColor)
                                .padding(8)
                        }
                    }

                    HStack {
                        Spacer()
                        QRView(text: accountKey)
                        Spacer()
                    }
                    .padding(.top, 5)
                }
                .padding(.vertical, 5)
            }
        }
        #if os(iOS)
            .navigationTitle("Account Keys")
            .navigationBarTitleDisplayMode(.large)
        #else
            .formStyle(.grouped)
        #endif
    }
}

#Preview {
    NavigationStack {
        SettingsView()
    }
    .environment(BillingState.preview)
}
