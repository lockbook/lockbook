import SwiftUI
import SwiftLockbookCore

struct BookView: View {
    @EnvironmentObject var onboarding: OnboardingService
    
    let currentFolder: DecryptedFileMetadata
    let account: Account
    
    @State var moving: DecryptedFileMetadata?
    
    #if os(iOS)
    @Environment(\.horizontalSizeClass) var horizontal
    @Environment(\.verticalSizeClass) var vertical
    #endif

    var body: some View {
        platformFileTree
            .sheet(isPresented: $onboarding.anAccountWasCreatedThisSession, content: { BeforeYouStart() })
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
    }
    
    var iPad: some View {
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
    }
#endif

    var macOS: some View {
        NavigationView {
            FileListView(currentFolder: currentFolder, account: account)
        }
    }
    
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

struct BookView_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            BookView(currentFolder: FakeApi.root, account: .fake(username: "jeff"))
                .ignoresSafeArea()
        }
    }
}
