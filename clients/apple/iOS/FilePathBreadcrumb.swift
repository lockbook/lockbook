import Foundation
import SwiftUI
import SwiftLockbookCore

struct FilePathBreadcrumb: View {
    
    @EnvironmentObject var fileService: FileService
    
    var body: some View {
        ScrollViewReader { scrollHelper in
            ScrollView(.horizontal, showsIndicators: false) {
                HStack {
                    if(fileService.path.count - 2 >= 0) {
                        breadcrumb
                    }
                }
                .onChange(of: fileService.path.count) { count in
                    if count > 0 {
                        withAnimation {
                            scrollHelper.scrollTo(fileService.path.count - 2, anchor: .trailing)
                        }
                    }
                }
            }
        }
        .padding(.horizontal)
    }
    
    var breadcrumb: some View {
        ForEach(0...fileService.path.count - 2, id: \.self) { index in
            let lastFileIndex = fileService.path.count - 2
            let file = fileService.path[index];

            if(index == lastFileIndex) {
                Button(action: {
                    withAnimation {
                        fileService.pathBreadcrumbClicked(file)
                    }
                }, label: {
                    Image(systemName: "folder.fill")
                        .foregroundColor(.blue)
                    Text(file.name)
                })
                .padding(.trailing)
                .id(index)
            } else {
                Button(action: {
                    withAnimation {
                        fileService.pathBreadcrumbClicked(file)
                    }
                }, label: {
                    Image(systemName: "folder.fill")
                        .foregroundColor(.blue)
                    Text(file.name)
                })
                .id(index)
            }
            
            if(lastFileIndex != index) {
                Image(systemName: "chevron.right")
                    .foregroundColor(.gray)
            }
        }
    }
}

struct FilePathBreadcrumb_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            FileListView()
                .mockDI()
        }
    }
}
