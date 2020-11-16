import SwiftUI
import SwiftLockbookCore

struct FileListView: View {
    @ObservedObject var core: Core
    let account: Account
    @State var selectedFile: FileMetadataWithChildren?
    @State var showingAccount: Bool = false
    @State var showingActions: Bool = false

    var body: some View {
        let baseView = List {
            OutlineGroup(core.grouped, children: \.children) { meta in
                if meta.meta.fileType == .Folder {
                    FileCell(meta: meta.meta)
                        .foregroundColor(selectedFile.map({ $0.id == meta.id }) ?? false ? .accentColor : .primary)
                        .onLongPressGesture {
                            selectedFile = meta
                            showingActions = true
                        }
                } else {
                    NavigationLink(destination: EditorView(core: core, meta: meta.meta).equatable()) {
                        FileCell(meta: meta.meta)
                            .foregroundColor(selectedFile.map({ $0.id == meta.id }) ?? false ? .accentColor : .primary)
                    }
                    .onLongPressGesture {
                        selectedFile = meta
                        showingActions = true
                    }
                }
            }
            HStack {
                Spacer()
                Text("\(core.files.count) items")
                    .foregroundColor(.secondary)
                Spacer()
            }
        }
        .navigationTitle("\(account.username)'s files")
        .onReceive(core.timer, perform: { _ in
            core.sync()
        })
        .popover(isPresented: $showingActions, content: {
            ActionsView(core: core, maybeSelected: selectedFile)
                .padding()
        })

        #if os(iOS)
        return baseView
            .navigationBarItems(leading: HStack {
                Button(action: { showingAccount.toggle() }) {
                    Image(systemName: "person.circle.fill")
                }
                .sheet(isPresented: $showingAccount, content: {
                    AccountView(core: core, account: account)
                })
                Button(action: { showingActions.toggle() }) {
                    Image(systemName: "plus.circle")
                }
            }, trailing: HStack {
                Button(action: core.sync) {
                    SyncIndicator(syncing: $core.syncing)
                        .foregroundColor(core.syncing ? .pink : .accentColor)
                }
                .disabled(core.syncing)
            })
        #else
        return baseView
            .toolbar {
                HStack {
                    Button(action: core.sync) {
                        SyncIndicator(syncing: $core.syncing)
                    }.font(.title)
                    .disabled(core.syncing)
                    Button(action: { showingActions.toggle() }) {
                        Image(systemName: "plus.circle")
                    }.font(.title)
                }
            }
        #endif
    }
}

struct FileListView_Previews: PreviewProvider {
    static let core = Core()

    static var previews: some View {
        FileListView(core: core, account: core.account!)
    }
}
