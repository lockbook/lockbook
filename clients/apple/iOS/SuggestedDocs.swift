import SwiftUI
import SwiftLockbookCore

struct SuggestedDocs: View {
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var current: CurrentDocument

    var isiOS: Bool
    
    var body: some View {
        ScrollView(.horizontal) {
            LazyHStack {
                if let suggestedDocs = fileService.suggestedDocs {
                    if !suggestedDocs.isEmpty {
                        ForEach(suggestedDocs) { meta in
                            if isiOS {
                                NavigationLink(destination: DocumentView(meta: meta)) {
                                    iOSSuggestedDocCell(name: meta.name, parentName: "\(fileService.idsAndFiles[meta.parent]!.name)/", duration: meta.lastModified)
                                        .padding(.trailing, 5)
                                }
                            } else {
                                HStack {
                                    Button(action: {
                                        current.selectedDocument = meta
                                    }) {
                                        iPadSuggestedDocCell(name: meta.name, parentName: "\(fileService.idsAndFiles[meta.parent]!.name)/", duration: meta.lastModified)
                                    }
                                    
                                    if meta != suggestedDocs.last {
                                        Divider()
                                            .padding(.vertical, 10)
                                    }
                                }
                            }
                        }
                    }
                } else {
                    ForEach(0...2, id: \.self) { index in
                        if isiOS {
                            iOSSuggestedDocLoadingCell()
                        } else {
                            iPadSuggestedDocLoadingCell()
                            
                            if index != 2 {
                                Divider()
                                    .padding(.vertical, 10)
                            }
                        }
                    }
                }
            }
            .setiOSOriPadOSSearchFrameing(isiOS: isiOS)
            
        }
        .listRowBackground(Color.clear)
        .listRowInsets(EdgeInsets())
    }
}

extension LazyHStack {
    @ViewBuilder
    func setiOSOriPadOSSearchFrameing(isiOS: Bool) -> some View {
        if isiOS {
            self
        } else {
            self.frame(height: 120)
        }
    }
}

struct iOSSuggestedDocCell: View {
    let name: String
    let parentName: String
    
    let duration: UInt64
    
    var body: some View {
        VStack(alignment: .leading) {
            Text(name)
            
            HStack {
                Text(parentName)
                    .font(.caption)
                    .foregroundColor(.accentColor)
                
                Spacer()
                
                Text(timeAgo(epoch: duration))
                    .font(.caption)
                    .foregroundColor(.gray)
            }
            .padding(.top, 1)
        }
        .padding(12)
        .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
        .background(RoundedRectangle(cornerRadius: 10).fill(.white))
    }
}

struct iOSSuggestedDocLoadingCell: View {
    var body: some View {
        VStack(alignment: .leading) {
            RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                .fill(.gray)
                .opacity(0.1)
                .cornerRadius(5)
                .frame(width: 70, height: 22)
            
            HStack {
                RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                    .fill(.gray)
                    .opacity(0.1)
                    .cornerRadius(5)
                    .frame(width: 100, height: 22)
                
                Spacer()
                
                RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                    .fill(.gray)
                    .opacity(0.1)
                    .cornerRadius(5)
                    .frame(width: 70, height: 22)
            }
            .padding(.top, 1)
        }
        .padding(12)
        .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
        .background(RoundedRectangle(cornerRadius: 10).fill(.white))
    }
}

struct iPadSuggestedDocCell: View {
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

struct iPadSuggestedDocLoadingCell: View {
    var body: some View {
        VStack(alignment: .leading) {
            RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                .fill(.gray)
                .opacity(0.1)
                .cornerRadius(5)
                .frame(width: 40, height: 22)
            
            RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                .fill(.gray)
                .opacity(0.1)
                .cornerRadius(5)
                .frame(width: 100, height: 18)
            
            RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                .fill(.gray)
                .opacity(0.1)
                .cornerRadius(5)
                .frame(width: 60, height: 18)
            
            HStack {
                Spacer()
                RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                    .fill(.gray)
                    .opacity(0.1)
                    .cornerRadius(5)
                    .frame(width: 60, height: 18)
            }
            .padding(.top, 5)
        }
        .frame(width: 100, height: 70)
        .padding(.horizontal)
        .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
    }
}
