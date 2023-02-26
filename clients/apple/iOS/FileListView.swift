import SwiftUI
import Foundation

struct FileListView: View {
    
    @EnvironmentObject var current: CurrentDocument
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var fileService: FileService
    
    var body: some View {
        NavigationView {
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
                    List(fileService.children) { meta in
                        FileCell(meta: meta)
                            .id(UUID())
                    }
                    HStack {
                        BottomBar(onCreating: {
                            if let parent = fileService.parent {
                                sheets.creatingInfo = CreatingInfo(parent: parent, child_type: .Document)
                            }
                        })
                    }
                    .navigationBarTitle(fileService.parent.map{($0.name)} ?? "")
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
        }.navigationViewStyle(.stack)
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
