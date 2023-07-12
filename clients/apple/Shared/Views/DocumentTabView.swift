import Foundation
import SwiftUI

struct DocumentTabView: View {
    
    var isiOS: Bool = false

    @EnvironmentObject var current: DocumentService
    @EnvironmentObject var files: FileService
    
    @State var docTabKillOpacity: [UUID: Double] = [:]

    var body: some View {
        VStack(spacing: 0) {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 0) {
                    ForEach(current.openDocumentsKeyArr, id: \.self) { id in
                        Button(action: {
                            current.selectedDoc = id
                            current.openDocuments[id]?.textDocument?.focused = true
                            docTabKillOpacity[id] = 1
                        }, label: {
                            HStack {
                                Image(systemName: documentExtensionToImage(name: DI.files.idsAndFiles[id]?.name ?? ""))
                                    .foregroundColor(current.selectedDoc == id ? .accentColor : .primary)
                                
                                Text(DI.files.idsAndFiles[id]?.name ?? "deleted")
                                    .foregroundColor(current.selectedDoc == id ? .accentColor : .primary)
                                    .font(.callout)
                                                                
                                Button(action: {
                                    if current.selectedDoc == id {
                                        current.selectedDoc = nil
                                    }
                                    
                                    docTabKillOpacity[id] = nil
                                    current.openDocuments[id] = nil
                                }, label: {
                                    Image(systemName: "xmark")
                                        .imageScale(.small)
                                        .foregroundColor(current.selectedDoc == id ? .accentColor : .primary)
                                })
                                .buttonStyle(.borderless)
                                .opacity(isiOS ? 1 : (docTabKillOpacity[id] ?? 0))
                                
//                                Divider()
//                                    .frame(height: 15)
//                                    .opacity(current.selectedDoc == id ? 0 : 1)
                            }
                            .padding(.vertical, 10)
                            .padding(.horizontal, 14)
                            .contentShape(Rectangle())
                        })
                        .buttonStyle(.borderless)
                        .onHover(perform: { hover in
                            withAnimation(.linear(duration: 0.1)) {
                                if hover {
                                    docTabKillOpacity[id] = 1
                                } else {
                                    docTabKillOpacity[id] = 0
                                }
                            }
                        })
                        .background(current.selectedDoc == id ? .blue.opacity(0.2) : .clear)
                    }
                    
                    Spacer()
                }
            }
            
            if !current.openDocuments.isEmpty {
                Divider()
                
                ZStack {
                    ForEach(Array(current.openDocuments.keys), id: \.self) { id in
                        DocumentView(id: id)
                            .opacity(id == current.selectedDoc ? 1 : 0)
                    }
                }

            } else {
                NoTabsView()
            }
        }
    }
}

struct NoTabsView: View {
    
    var body: some View {
        VStack() {
            Spacer()
                        
            Text("You have no open documents.")
                .noTabTextFormatting()
                .padding(.bottom, 3)
            
            Text("You can access all of your files in the left sidebar.")
                .noTabTextFormatting()
            
            VStack(alignment: .leading, spacing: 4) {
#if os(iOS)
                Text("Create a new document: \(Text(verbatim: "`Cmd+N`").noTabTextFormatting(true))")
                    .noTabTextFormatting()
                
                Text("Create a new drawing: \(Text(verbatim: "`Cmd+Ctrl+N`").noTabTextFormatting(true))")
                    .noTabTextFormatting()
                
                Text("Create a new folder: \(Text(verbatim: "`Cmd+Shift+N`").noTabTextFormatting(true))")
                    .noTabTextFormatting()
#else
                Text("Create a new document: \(Text(verbatim: "`Cmd+N`").noTabTextFormatting(true))")
                    .noTabTextFormatting()
                
                Text("Create a new folder: \(Text(verbatim: "`Cmd+Shift+N`").noTabTextFormatting(true))")
                    .noTabTextFormatting()
#endif

            }
            .padding()
            
            Spacer()
        }
    }
    
    
    
}

extension Text {
    
    func noTabTextFormatting(_ isCode: Bool = false) -> Text {
        if isCode {
            #if os(iOS)
            return self
                .font(.title3)
                .foregroundColor(.red)
            #else
            return self
                .font(.title3)
                .fontDesign(.monospaced)
                .foregroundColor(.red)
            #endif
        } else {
            return self
                .font(.title2)
                .foregroundColor(.gray)
                .fontWeight(.medium)
            
        }
    }
}
