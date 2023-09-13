import SwiftUI
import SwiftLockbookCore
import AlertToast
import Introspect

struct BookView: View {

    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var onboarding: OnboardingService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var share: ShareService
    @EnvironmentObject var search: SearchService

    let currentFolder: File
    let account: Account
    
    #if os(iOS)
    @Environment(\.horizontalSizeClass) var horizontal
    @Environment(\.verticalSizeClass) var vertical
    #endif
    
    var body: some View {
        platformFileTree
            .iOSOnlySheet(isPresented: $sheets.moving)
            .sheet(isPresented: $onboarding.anAccountWasCreatedThisSession, content: BeforeYouStart.init)
            .sheet(isPresented: $sheets.sharingFile, content: ShareFileSheet.init)
            .sheet(isPresented: $sheets.creatingFolder, content: NewFolderSheet.init)
            .toast(isPresenting: Binding(get: { files.successfulAction != nil }, set: { _ in files.successfulAction = nil }), duration: 2, tapToDismiss: true) {
                postFileAction()
            }
    }
    
    func postFileAction() -> AlertToast {
        if let action = files.successfulAction {
            switch action {
            case .delete:
                return AlertToast(type: .regular, title: "File deleted")
            case .move:
                return AlertToast(type: .regular, title: "File moved")
            case .createFolder:
                return AlertToast(type: .regular, title: "Folder created")
            case .importFiles:
                return AlertToast(type: .regular, title: "Imported successfully")
            }
        } else {
            return AlertToast(type: .regular, title: "ERROR")
        }
    }
    
    #if os(iOS)
    var iOS: some View {
        NavigationView {
            FileListView()
                .toolbar {
                    ToolbarItemGroup {
                        NavigationLink(
                            destination: PendingSharesView()) {
                                pendingShareToolbarIcon(isPendingSharesEmpty: share.pendingShares.isEmpty)
                            }
                        
                        NavigationLink(
                            destination: SettingsView().equatable(), isActive: $onboarding.theyChoseToBackup) {
                                Image(systemName: "gearshape.fill").foregroundColor(.blue)
                                    .padding(.horizontal, 10)
                            }
                    }
                }
        }
            .navigationViewStyle(.stack)
    }

    @ViewBuilder
    var iPad: some View {
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
            ZStack {
                iPad
                
                if search.pathSearchState != .NotSearching {
                    SearchActionBar()
                }
            }
        } else {
            iOS
        }
        #else
        ZStack {
            macOS
            
            if search.pathSearchState != .NotSearching {
                SearchActionBar()
            }
        }
        #endif
    }
}

extension View {
    func iOSOnlySheet(isPresented: Binding<Bool>) -> some View {
        #if os(iOS)
        self.sheet(isPresented: isPresented, content: MoveSheet.init)
        #else
        self
        #endif
    }
    
    #if os(iOS)
    func exportFileAndShowShareSheet(meta: File) {
        DispatchQueue.global(qos: .userInitiated).async {
            if let url = DI.importExport.exportFilesToTempDirSync(meta: meta) {
                DispatchQueue.main.async {
                    let activityVC = UIActivityViewController(activityItems: [url], applicationActivities: nil)
                    
                    if UIDevice.current.userInterfaceIdiom == .pad {
                        let thisViewVC = UIHostingController(rootView: self)
                        activityVC.popoverPresentationController?.sourceView = thisViewVC.view
                    }
                    
                    UIApplication.shared.connectedScenes.flatMap {($0 as? UIWindowScene)?.windows ?? []}.first {$0.isKeyWindow}?.rootViewController?.present(activityVC, animated: true, completion: nil)
                }
            }
        }
    }
    #endif
}

#if os(macOS)

extension NSView {
    func exportFileAndShowShareSheet(meta: File) {
        DispatchQueue.global(qos: .userInitiated).async {
            if let url = DI.importExport.exportFilesToTempDirSync(meta: meta) {
                DispatchQueue.main.async {
                    NSSharingServicePicker(items: [url]).show(relativeTo: .zero, of: self, preferredEdge: .minX)
                }
            }
        }
    }
}
#endif

@ViewBuilder
func pendingShareToolbarIcon(isPendingSharesEmpty: Bool) -> some View {
    #if os(iOS)
        ZStack {
            Image(systemName: "person.2.fill")
                .foregroundColor(.blue)
                                        
            if !isPendingSharesEmpty {
                Circle()
                    .foregroundColor(.red)
                    .frame(width: 12, height: 12)
                    .offset(x: 12, y: 5)
            }
        }
    #else
        ZStack {
            Image(systemName: "person.2.fill")
                .foregroundColor(.blue)
                                        
            if !isPendingSharesEmpty {
                Circle()
                    .foregroundColor(.red)
                    .frame(width: 7, height: 7)
                    .offset(x: 7, y: 3)
            }
        }
    #endif
}

struct BookView_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            BookView(currentFolder: FakeApi.root, account: .fake(username: "jeff"))
                    .ignoresSafeArea()
        }
    }
}

struct SearchActionBar: View {
    @State var text: String = ""
    
    @EnvironmentObject var search: SearchService
    
    var body: some View {
        Group {
            Rectangle()
                .onTapGesture {
                    search.pathSearchState = .NotSearching
                }
                .foregroundColor(.gray.opacity(0.01))
            
            GeometryReader { geometry in
                VStack {
                    VStack {
                        HStack {
                            Image(systemName: "magnifyingglass")
                            
                            TextField("Open quickly", text: $text)
                                .textFieldStyle(.plain)
                                .introspectTextField { textField in
                                    textField.becomeFirstResponder()
                                }
                        }
                        
                        if case .SearchSuccessful(let paths) = search.pathSearchState {
                            Divider()
                                .padding(.top)
                            
                            ScrollViewReader { scrollHelper in
                                ScrollView {
                                    ForEach(Array(zip(paths.indices, paths)), id: \.0) { index, path in
                                        SearchResultCellView(name: path.getNameAndPath().name, path: path.getNameAndPath().path, matchedIndices: path.matchedIndices, index: index, selected: search.pathSearchSelected)
                                    }
                                    .scrollIndicators(.visible)
                                    .padding(.horizontal)
                                }
                                .frame(maxHeight: 300)
                                .onChange(of: search.pathSearchSelected) { newValue in
                                    withAnimation {
                                        scrollHelper.scrollTo(newValue, anchor: .center)
                                    }
                                }
                            }
                        } else if search.pathSearchState == .Searching {
                            ProgressView()
                                .imageScale(.small)
                        }
                    }
                    .padding()
                    .background(
                        RoundedRectangle(cornerSize: CGSize(width: 20, height: 20))
                            .foregroundColor(.white)
                            .shadow(radius: 10)
                    )
                    .frame(width: 500)
                    .onChange(of: text, perform: { newValue in
                        DispatchQueue.main.async {
                            search.asyncSearchFilePath(input: newValue)
                        }
                    })
                }
                .padding(.top, geometry.size.height / 4.5)
                .padding(.leading, (geometry.size.width / 2) - 250)
                .onAppear {
                    #if os(macOS)
                    NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { nsevent in
                        DispatchQueue.global(qos: .userInitiated).async {
                            if case .SearchSuccessful(let paths) = search.pathSearchState {
                                if nsevent.keyCode == 125 {
                                    print("down \(search.pathSearchSelected) \(min(paths.count - 1, search.pathSearchSelected + 1))")
                                    search.pathSearchSelected = min(paths.count - 1, search.pathSearchSelected + 1)
                                } else {
                                    if nsevent.keyCode == 126 {
                                        print("down \(search.pathSearchSelected) \(max(0, search.pathSearchSelected - 1))")
                                        
                                        search.pathSearchSelected = max(0, search.pathSearchSelected - 1)
                                    }
                                }
                                
                                if nsevent.keyCode == 36 {
                                    search.openPathAtIndex(index: search.pathSearchSelected)
                                }
                            }
                            
                            if nsevent.keyCode == 53 {
                                search.pathSearchState = .NotSearching
                            }
                        }

                        return nsevent
                    }
                    #endif
                }
            }
        }
    }
}

struct PathSearchResultView: View {
    var name: String
    var path: String
    
    var body: some View {
        Text(name)
        Text(path)
    }
}
