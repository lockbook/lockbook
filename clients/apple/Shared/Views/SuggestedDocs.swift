import SwiftUI
import SwiftLockbookCore

struct SuggestedDocs: View {
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var current: DocumentService
    
    var isiOS: Bool = false
    
    @State var branchState: Bool = true
    
    var body: some View {
        #if os(iOS)
        iOSSuggestedDocs
        #elseif os(macOS)
        macOSSuggestedDocs
        #endif
    }
    
    #if os(iOS)
    var iOSSuggestedDocs: some View {
        Group {
            if isiOS {
                iOSSuggestedDocsBase
            } else {
                if fileService.suggestedDocs?.isEmpty != true {
                    Button(action: {
                        withAnimation {
                            branchState.toggle()
                        }
                    }) {
                        HStack {
                            Text("Suggested")
                                .bold()
                                .foregroundColor(.primary)
                                .textCase(.none)
                                .font(.headline)
                            
                            Spacer()
                            
                            if branchState {
                                Image(systemName: "chevron.down")
                                    .foregroundColor(.gray)
                                    .imageScale(.small)
                            } else {
                                Image(systemName: "chevron.right")
                                    .foregroundColor(.gray)
                                    .imageScale(.small)
                            }
                        }
                        .padding(.top)
                        .padding(.bottom, 5)
                        .contentShape(Rectangle())
                    }
                    
                    if branchState {
                        iOSSuggestedDocsBase
                        Spacer()
                    } else {
                        Spacer()
                    }
                }
            }
        }
    }
    
    var iOSSuggestedDocsBase: some View {
        ScrollView(.horizontal) {
            HStack {
                if let suggestedDocs = fileService.suggestedDocs {
                    ForEach(suggestedDocs) { meta in
                        if let parentMeta = fileService.idsAndFiles[meta.parent] {
                            if isiOS {
                                NavigationLink(destination: DocumentView(model: current.openDoc(meta: meta))) {
                                    SuggestedDocCell(name: meta.name, parentName: "\(parentMeta.name)/", duration: meta.lastModified, isiOS: isiOS)
                                        .padding(.trailing, 5)
                                }
                            } else {
                                HStack {
                                    Button(action: {
                                        current.openDocuments[meta.id] = DocumentLoadingInfo(meta)
                                    }) {
                                        SuggestedDocCell(name: meta.name, parentName: "\(parentMeta.name)/", duration: meta.lastModified, isiOS: isiOS)
                                    }
                                }
                            }
                        }
                    }
                } else {
                    ForEach(0...2, id: \.self) { index in
                        SuggestedDocLoadingCell(isiOS: isiOS)
                    }
                }
            }
            .setSuggestedDocsFraming(isiOS: isiOS)
        }
        .listRowBackground(Color.clear)
        .listRowInsets(EdgeInsets())
    }
    
    #else
    var macOSSuggestedDocs: some View {
        Group {
            if fileService.suggestedDocs?.isEmpty != true {
                VStack {
                    Button(action: {
                        withAnimation {
                            branchState.toggle()
                        }
                    }) {
                        HStack {
                            Text("Suggested")
                                .bold()
                                .foregroundColor(.gray)
                                .font(.subheadline)
                            Spacer()
                            if branchState {
                                Image(systemName: "chevron.down")
                                    .foregroundColor(.gray)
                                    .imageScale(.small)
                            } else {
                                Image(systemName: "chevron.right")
                                    .foregroundColor(.gray)
                                    .imageScale(.small)
                            }
                        }
                        .padding(.horizontal)
                        .contentShape(Rectangle())
                    }
                    
                    if branchState {
                        ScrollView(.horizontal) {
                            HStack {
                                if let suggestedDocs = fileService.suggestedDocs {
                                    ForEach(suggestedDocs) { meta in
                                        if let parentMeta = fileService.idsAndFiles[meta.parent] {
                                            Button(action: {
                                                current.openDocuments.removeAll()
                                                let _ = current.openDoc(meta: meta)
                                            }) {
                                                SuggestedDocCell(name: meta.name, parentName: "\(parentMeta.name)/", duration: meta.lastModified)
                                            }
                                        }
                                    }
                                } else {
                                    ForEach(0...2, id: \.self) { index in
                                        SuggestedDocLoadingCell()
                                    }
                                }
                            }
                            .setSuggestedDocsFraming(isiOS: isiOS)
                        }
                        .listRowBackground(Color.clear)
                        .listRowInsets(EdgeInsets())
                    }
                }
            }
        }
    }
    #endif
}

extension HStack {
    @ViewBuilder
    func setSuggestedDocsFraming(isiOS: Bool) -> some View {
        #if os(iOS)
        if isiOS {
            self
                .frame(height: 75)
        } else {
            self
                .frame(height: 80)
                .padding(.horizontal)
        }
        #elseif os(macOS)
        self
            .frame(height: 65)
            .padding(.horizontal)
        #endif
    }
}

extension View {
    @ViewBuilder
    func setSuggestedDocsBackground(isiOS: Bool, colorScheme: ColorScheme) -> some View {
        #if os(iOS)
        if isiOS {
            let fill: Color = colorScheme == .light ? .white : .blue.opacity(0.19)
            
            self.background(RoundedRectangle(cornerRadius: 10).fill(fill))
        } else {
            let fill: Color = colorScheme == .light ? .blue.opacity(0.08) : .blue.opacity(0.19)

            self.background(RoundedRectangle(cornerRadius: 10).fill(fill))
        }
        #else
        let fill: Color = colorScheme == .light ? .white.opacity(0.5) : .blue.opacity(0.19)
        
        self.background(RoundedRectangle(cornerRadius: 10).fill(fill))
        #endif
    }
}

struct SuggestedDocCell: View {
    let name: String
    let parentName: String
    
    let duration: UInt64
    
    var isiOS: Bool = false
    
    @Environment(\.colorScheme) var colorScheme
    
    var body: some View {
        VStack(alignment: .leading) {
            
            Text(name)
            
            HStack {
                Text(parentName)
                    .font(.caption)
                    .foregroundColor(.accentColor)
                
                Spacer()
                
                Text(DI.core.timeAgo(timeStamp: Int64(duration)))
                    .font(.caption)
                    .foregroundColor(.gray)
            }
            .padding(.top, 1)
        }
        .padding(12)
        .contentShape(Rectangle())
        .setSuggestedDocsBackground(isiOS: isiOS, colorScheme: colorScheme)
    }
}

struct SuggestedDocLoadingCell: View {
    
    var isiOS: Bool = false
    
    @Environment(\.colorScheme) var colorScheme
    
    var body: some View {
        VStack(alignment: .leading) {
            RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                .fill(.gray)
                .opacity(0.1)
                .cornerRadius(5)
                .frame(width: 70, height: 16)
            
            HStack {
                RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                    .fill(.gray)
                    .opacity(0.1)
                    .cornerRadius(5)
                    .frame(width: 70, height: 16)
                
                Spacer()
                
                RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                    .fill(.gray)
                    .opacity(0.1)
                    .cornerRadius(5)
                    .frame(width: 40, height: 16)
            }
            .padding(.top, 1)
        }
        .padding(12)
        .contentShape(Rectangle())
        .setSuggestedDocsBackground(isiOS: isiOS, colorScheme: colorScheme)
    }
}
