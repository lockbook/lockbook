import SwiftUI
import Foundation

struct FileListView: View {
    
    @EnvironmentObject var current: CurrentDocument
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var fileService: FileService
    
    var body: some View {
            ZStack {
                VStack {
                    if let newDoc = sheets.created, newDoc.fileType == .Document {
                        NavigationLink(destination: DocumentView(meta: newDoc), isActive: Binding(get: { current.selectedDocument != nil }, set: { _ in current.selectedDocument = nil }) ) {
                             EmptyView()
                         }
                         .hidden()
                    }
                    
                    List(fileService.childrenOfParent()) { meta in
                        FileCell(meta: meta)
                            .id(UUID())
                    }
                    .navigationBarTitle(fileService.parent.map{($0.name)} ?? "")
                    
                    FilePathBreadcrumb()
                    
                    HStack {
                        BottomBar(onCreating: {
                            if let parent = fileService.parent {
                                sheets.creatingInfo = CreatingInfo(parent: parent, child_type: .Document)
                            }
                        })
                    }
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
            .gesture(
                DragGesture().onEnded({ (value) in
                    if value.translation.width > 50 && fileService.parent?.isRoot == false {
                        withAnimation {
                            fileService.upADirectory()
                        }
                    }
                }))
    }
}

struct FileListView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            FileListView()
                .mockDI()
        }
    }
}
