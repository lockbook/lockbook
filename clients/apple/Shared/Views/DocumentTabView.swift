import Foundation
import SwiftUI
#if os(macOS)
import AppKit
#endif

struct DocumentTabView: View {
    
    var isiOS: Bool = false

    @EnvironmentObject var current: DocumentService
    @EnvironmentObject var files: FileService
    
    @State var docTabKillOpacity: [UUID: Double] = [:]

    var body: some View {
        VStack(spacing: 0) {
            ScrollViewReader { scrollHelper in
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 0) {
                        ForEach(current.openDocumentsKeyArr, id: \.hashValue) { id in
                            VStack {
                                Button(action: {
                                    current.selectedDoc = id
                                    current.openDocuments[id]?.textDocument?.shouldFocus = true
                                    docTabKillOpacity[id] = 1
                                }, label: {
                                    HStack {
                                        Image(systemName: documentExtensionToImage(name: DI.files.idsAndFiles[id]?.name ?? ""))
                                            .foregroundColor(current.selectedDoc == id ? .accentColor : .primary)
                                        
                                        Text(DI.files.idsAndFiles[id]?.name ?? "deleted")
                                            .foregroundColor(current.selectedDoc == id ? .accentColor : .primary)
                                            .font(.callout)
                                        
                                        Button(action: {
                                            current.closeDoc(id)
                                            docTabKillOpacity[id] = nil
                                        }, label: {
                                            Image(systemName: "xmark")
                                                .imageScale(.small)
                                                .foregroundColor(current.selectedDoc == id ? .accentColor : .primary)
                                        })
                                        .buttonStyle(.borderless)
                                        .opacity(isiOS ? 1 : (docTabKillOpacity[id] ?? 0))
                                        
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
                            .middleClickClose(id: id)
                        }
                        
                        Spacer()
                    }
                }
                .onChange(of: current.selectedDoc) { maybeSelectedDoc in
                    if let id = maybeSelectedDoc {
                        withAnimation {
                            scrollHelper.scrollTo(id.hashValue, anchor: .trailing)
                        }
                    }
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
    
    
    var logo: Image {
#if os(iOS)
        Image(uiImage: UIImage(named: "logo")!)
#else
        Image(nsImage: NSImage(named: NSImage.Name("logo"))!)
#endif
    }
    
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
                Text("Create a new document: ⌘N")
                    .noTabTextFormatting()
                
                Text("Create a new drawing: ⌘⌃N")
                    .noTabTextFormatting()
                
                Text("Create a new folder: ⌘⇧N")
                    .noTabTextFormatting()
#else
                Text("Create a new document: ⌘N")
                    .noTabTextFormatting()
                
                Text("Create a new folder: ⌘⇧N")
                    .noTabTextFormatting()
#endif
                
            }
            .padding()
            
            Spacer()
        }
    }
}

extension Text {
    func noTabTextFormatting() -> Text {
        return self
            .font(.title2)
            .foregroundColor(.gray)
            .fontWeight(.medium)
    }
}

extension View {
    func middleClickClose(id: UUID) -> some View {
        #if os(iOS)
        return self
        #else
        return self.background(MiddleClickRecognizer(id: id))
        #endif
    }
}

#if os(macOS)
struct MiddleClickRecognizer: NSViewRepresentable {
    let id: UUID
    
    func makeNSView(context: Context) -> NSView {
        MiddleClickView(id: id)
    }
    
    func updateNSView(_ nsView: NSView, context: Context) {}
}

class MiddleClickView: NSView {
    let id: UUID
    
    init(id: UUID) {
        self.id = id
        super.init(frame: .zero)
    }
    
    // will never be called
    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    override func otherMouseDown(with event: NSEvent) {
        if event.buttonNumber == 2 {
            DI.currentDoc.closeDoc(id)
        }
    }
}


#endif
