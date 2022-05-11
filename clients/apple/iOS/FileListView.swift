import SwiftUI
import SwiftLockbookCore
import PencilKit

struct FileListView: View {
    
    let currentFolder: DecryptedFileMetadata
    let account: Account
    
    @State var creatingFile: Bool = false
    @State var creating: FileType?
    @State var creatingName: String = ""
    @State private var selection: DecryptedFileMetadata?
    @State private var newFile: DecryptedFileMetadata?
    
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var errors: UnexpectedErrorService
    var files: [DecryptedFileMetadata] {
        fileService.files.filter {
            $0.parent == currentFolder.id && $0.id != currentFolder.id
        }
    }
    
    var body: some View {
        ZStack {
            // The whole active selection concept doesn't handle links that don't exist yet properly
            // This is a workaround for that scenario.
            if let newDoc = newFile, newDoc.fileType == .Document {
                NavigationLink(destination: DocumentView(meta: newDoc), isActive: Binding.constant(true)) {
                    EmptyView()
                }.hidden()
            }
            
            if let newFolder = newFile, newFolder.fileType == .Folder {
                NavigationLink(
                    destination: FileListView(currentFolder: newFolder, account: account), isActive: Binding.constant(true)) {
                        EmptyView()
                    }.isDetailLink(false)
                    .hidden()
            }
            
            VStack {
                List (files) { meta in
                    FileCell(meta: meta, selection: $selection)
                }
                HStack {
                    BottomBar(onCreating: { creatingFile = true })
                }
                .navigationBarTitle(currentFolder.decryptedName)
                .padding(.horizontal, 10)
                .sheet(isPresented: $creatingFile, onDismiss: {
                    self.selection = self.newFile
                }, content: {
                    NewFileSheet(parent: currentFolder, selection: $newFile)
                })
                .onChange(of: selection) {_ in
                    // When we return back to this screen, we have to change newFile back to nil regardless
                    // of it's present value, otherwise we won't be able to navigate to new, new files
                    if self.selection == nil { self.newFile = nil }
                }
            }
        }
        
    }
}

struct FileListView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            FileListView(currentFolder: Mock.files.root!, account: Mock.accounts.account!)
                .mockDI()
        }
    }
}
