import SwiftUI
import SwiftLockbookCore

struct SuggestedDocs: View {
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var current: CurrentDocument

    var isiOS: Bool
    
    var body: some View {
        if !fileService.suggestedDocs.isEmpty {
            VStack(alignment: .leading) {
                Text("**Suggested**")
                    .padding(.bottom)
                    .foregroundColor(.gray)
                    
                ScrollView(.horizontal) {
                    LazyHStack {
                        ForEach(fileService.suggestedDocs) { meta in
                            if isiOS {
                                NavigationLink(destination: DocumentView(meta: meta)) {
                                    iOSSuggestedDocCell(name: meta.name, parentName: "\(fileService.idsAndFiles[meta.parent]!.name)/", duration: meta.lastModified)
                                }
                            } else {
                                Button(action: {
                                    current.selectedDocument = meta
                                }) {
                                    iOSSuggestedDocCell(name: meta.name, parentName: "\(fileService.idsAndFiles[meta.parent]!.name)/", duration: meta.lastModified)
                                }
                            }
                        }
                    }
                }
                .frame(height: 120)
            }
            .padding(.horizontal)
        }
    }
}

struct iOSSuggestedDocCell: View {
    let name: String
    let parentName: String
    
    let duration: UInt64
    
    var body: some View {
        VStack(alignment: .leading) {
            Image(systemName: "doc.circle")
                .resizable()
                .scaledToFill()
                .frame(width: 21, height: 21)
                .foregroundColor(.accentColor)
            
            Text(parentName)
                .font(.callout)
                .foregroundColor(.gray)
            
            Text(name)
                .font(.callout)
            
            HStack {
                Spacer()
                Text(timeAgo(epoch: duration))
                    .foregroundColor(.gray)
                    .font(.callout)
            }
            .padding(.top, 5)
        }
        .frame(width: 100, height: 70)
        .padding(.horizontal)
        .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
    }
}
