import SwiftUI
import SwiftLockbookCore

struct BookView: View {

    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var onboarding: OnboardingService

    let currentFolder: File
    let account: Account

    #if os(iOS)
    @Environment(\.horizontalSizeClass) var horizontal
    @Environment(\.verticalSizeClass) var vertical
    #endif

    var body: some View {
        platformFileTree
                .sheet(isPresented: $onboarding.anAccountWasCreatedThisSession, content: { BeforeYouStart() })
                .sheet(isPresented: $sheets.creating) {
                    NewFileSheet()
                }
                .iOSOnlySheet(isPresented: $sheets.moving)
                .sheet(isPresented: $sheets.renaming) {
                    RenamingSheet()
                }
    }

    #if os(iOS)
    var iOS: some View {
        NavigationView {
            FileListView(currentFolder: currentFolder, account: account)
                    .toolbar {
                        ToolbarItem(placement: .navigationBarTrailing) {
                            NavigationLink(
                                    destination: SettingsView().equatable(), isActive: $onboarding.theyChoseToBackup) {
                                Image(systemName: "gearshape.fill")
                                        .foregroundColor(.blue)
                            }
                        }
                    }
        }
                .navigationViewStyle(.stack)
    }

    @ViewBuilder
    var iPad: some View {
        let _ = print("Switching to iPad, \(DI.currentDoc.selectedItem)")
        NavigationView {
            FileTreeView(currentFolder: currentFolder, account: account)
        }
    }
    #else
    var macOS: some View {
        NavigationView {
            FileListView()
        }
    }
    #endif

    @ViewBuilder
    var platformFileTree: some View {
        #if os(iOS)
        if horizontal == .regular && vertical == .regular {
            iPad
        } else {
            iOS
        }
        #else
        macOS
        #endif
    }
}

extension View {
    func iOSOnlySheet(isPresented: Binding<Bool>) -> some View {
        #if os(iOS)
        self.sheet(isPresented: isPresented) {
            MoveSheet()
        }
        #else
        self
        #endif
    }
}

struct BookView_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            BookView(currentFolder: FakeApi.root, account: .fake(username: "jeff"))
                    .ignoresSafeArea()
        }
    }
}
