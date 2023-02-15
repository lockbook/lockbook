import SwiftUI
import SwiftLockbookCore
import PencilKit

struct FileListView: View {

    let currentFolder: File
    let account: Account

    @EnvironmentObject var current: CurrentDocument
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var errors: UnexpectedErrorService
    var files: [File] {
        fileService.childrenOf(currentFolder)
    }

    // There are too many workarounds here, we want to learn how to properly animate a list and then do this ourselves
    // So we can nicely navigate to new folders that have been created and manage the idea of a breadcrumb trail
    var body: some View {
        ZStack {
            // The whole active selection concept doesn't handle links that don't exist yet properly
            // This is a workaround for that scenario.
            if let newDoc = sheets.created, newDoc.fileType == .Document {
                NavigationLink(destination: DocumentView(meta: newDoc), isActive: Binding.constant(true)) {
                    EmptyView()
                }
                        .hidden()
            }


            VStack {
                List(files) { meta in
                    FileCell(meta: meta)
                }
                HStack {
                    BottomBar(onCreating: {
                        sheets.creatingInfo = CreatingInfo(parent: currentFolder, child_type: .Document)
                    })
                }
                        .navigationBarTitle(currentFolder.name)
                        .padding(.horizontal, 10)
                        .onReceive(current.$selectedDocument) { _ in
                            print("cleared")
                            // When we return back to this screen, we have to change newFile back to nil regardless
                            // of it's present value, otherwise we won't be able to navigate to new, new files
                            if current.selectedDocument == nil {
                                sheets.created = nil
                            }
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
