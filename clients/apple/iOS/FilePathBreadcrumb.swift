import Foundation
import SwiftUI
import SwiftLockbookCore

struct FilePathBreadcrumb: View {
    
    @EnvironmentObject var files: FileService
        
    var body: some View {
        ScrollViewReader { scrollHelper in
            ScrollView(.horizontal, showsIndicators: false) {
                HStack {
                    if(files.path.count > 0) {
                        breadcrumb
                    }
                }
                .onChange(of: files.path.count) { count in
                    if count > 0 {
                        withAnimation {
                            scrollHelper.scrollTo(files.path.count - 1, anchor: .trailing)
                        }
                    }
                }
            }
        }
        .padding(.horizontal)
    }
    
    var breadcrumb: some View {
        ForEach(0..<files.path.count, id: \.self) { index in
            let lastFileIndex = files.path.count - 1

            if index == 0 {
                Button(action: {
                    DI.files.path.removeAll()
                }, label: {
                    Image(systemName: "folder.fill")
                        .foregroundColor(.blue)
                    
                    Text(DI.accounts.account?.username ?? "...")
                        .font(.callout)
                })
                .id(index)
            } else {
                let file = files.path[index - 1]

                if index == lastFileIndex {
                    Button(action: {
                        DI.files.pathBreadcrumbClicked(file)
                    }, label: {
                        Image(systemName: "folder.fill")
                            .foregroundColor(.blue)
                        
                        Text(file.name)
                            .font(.callout)
                    })
                    .padding(.trailing)
                    .id(index)
                } else {
                    Button(action: {
                        DI.files.pathBreadcrumbClicked(file)
                    }, label: {
                        Image(systemName: "folder.fill")
                            .foregroundColor(.blue)
                        
                        Text(file.name)
                            .font(.callout)
                    })
                    .id(index)
                }
            }
            
            if lastFileIndex != index {
                Image(systemName: "chevron.right")
                    .foregroundColor(.gray)
            }
        }
    }
}
