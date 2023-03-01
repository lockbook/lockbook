import Foundation
import SwiftUI

struct FilePathBreadcrumb: View {
    
    @EnvironmentObject var fileService: FileService
    
    var body: some View {
        ScrollView(.horizontal) {
            HStack {
                ForEach(fileService.path) { file in
                    Button(action: {
                        withAnimation {
                            fileService.pathBreadcrumbClicked(file)
                        }
                    }, label: {
                        Text(file.name)
                    })
                    Image(systemName: "arrow.right")
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
