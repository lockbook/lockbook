import Foundation
import SwiftUI

struct FilePathBreadcrumb: View {
    
    @EnvironmentObject var fileService: FileService
    
    var body: some View {
        ScrollViewReader { scrollHelper in
            ScrollView(.horizontal, showsIndicators: false) {
                HStack {
                    ForEach(fileService.path, id: \.self) { file in                        
                        if(fileService.path.last == file) {
                            Button(action: {
                                withAnimation {
                                    fileService.pathBreadcrumbClicked(file)
                                }
                            }, label: {
                                Text(file.name)
                            })
                            .padding(.trailing)
                            .id(file)
                        } else {
                            Button(action: {
                                withAnimation {
                                    fileService.pathBreadcrumbClicked(file)
                                }
                            }, label: {
                                Text(file.name)
                            })
                            .id(file)
                        }
                        
                        if(fileService.path.last != file) {
                            Image(systemName: "arrow.forward")
                                .foregroundColor(.accentColor)
                        }
                        
                    }
                }
                .onChange(of: fileService.path.count) { count in
                    if count > 0 {
                        withAnimation {
                            scrollHelper.scrollTo(fileService.path.last, anchor: .trailing)
                        }
                    }
                }
            }
        }
        .padding(.horizontal)
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
