import Foundation
import SwiftLockbookCore
import Combine
import PencilKit
import SwiftUI
import SwiftEditor

class DocumentService: ObservableObject {
    
    @Published var openDocuments: [UUID : DocumentLoadingInfo] = [:]
    var openDocumentsKeyArr: [UUID] {
        get {
            Array(openDocuments.keys).sorted(by: { lhid, rhid in
                openDocuments[lhid]!.timeCreated < openDocuments[rhid]!.timeCreated
                
            })
        }
    }
    
    @Published var isPendingSharesOpen: Bool = false
    @Published var selectedFolder: File?
    
    @Published var selectedDoc: UUID?
    
    var justCreatedDoc: File? = nil
    var justOpenedLink: File? = nil

    func openDoc(id: UUID, isiPhone: Bool = false) {
        if openDocuments[id] == nil {
            openDocuments[id] = DocumentLoadingInfo(DI.files.idsAndFiles[id]!, isiPhone)
        }
    }
    
    func getDocInfoOrCreate(id: UUID, isiPhone: Bool = true) -> DocumentLoadingInfo {
        openDoc(id: id, isiPhone: isiPhone)
        
        return openDocuments[id]!
    }
    
    func cleanupOldDocs(_ isiPhone: Bool = false, _ oldId: UUID? = nil) {
        isPendingSharesOpen = false
        selectedDoc = nil

        if let id = oldId,
           openDocuments[id]?.dismissForLink != nil {
            openDocuments[id] = nil
        } else if isiPhone {
            justCreatedDoc = nil
            justOpenedLink = nil
            openDocuments.removeAll()
        }
    }
    
    func closeDoc(_ maybeId: UUID?) {
        if let id = maybeId {
            
            if id == selectedDoc {
                if openDocumentsKeyArr.firstIndex(of: id) == openDocumentsKeyArr.count - 1 {
                    selectPreviousOpenDoc()
                } else {
                    selectNextOpenDoc()
                }
                
                if id == selectedDoc {
                    selectedDoc = nil
                }
            }
            
            openDocuments[id] = nil
        }
    }
    
    func setSelectedOpenDocById(maybeId: UUID?) {
        selectedDoc = maybeId
        
        if let id = maybeId {
            openDocuments[id]?.textDocument?.shouldFocus = true
        }
    }
    
    // index must be greater than or equal to 0 and less than
    func selectOpenDocByIndex(index: Int) {
        if index >= 0 && index < 9 {
            if index == 8 {
                setSelectedOpenDocById(maybeId: openDocumentsKeyArr.last)
            }
            
            if index < openDocumentsKeyArr.count {
                setSelectedOpenDocById(maybeId: openDocumentsKeyArr[index])
            }
        }
    }
    
    func selectNextOpenDoc() {
        var selectedIndex = -1
        
        for (index, id) in openDocumentsKeyArr.enumerated() {
            if selectedDoc == id {
                selectedIndex = index
                
                break
            }
        }
        
        if selectedIndex == -1 {
            return
        }
        
        if selectedIndex + 1 >= openDocumentsKeyArr.count {
            setSelectedOpenDocById(maybeId: openDocumentsKeyArr.first)
        } else {
            setSelectedOpenDocById(maybeId: openDocumentsKeyArr[selectedIndex + 1])
        }
    }
    
    func selectPreviousOpenDoc() {
        var selectedIndex = -1
        
        for (index, id) in openDocumentsKeyArr.enumerated() {
            if selectedDoc == id {
                selectedIndex = index
                
                break
            }
        }
        
        if selectedIndex == -1 {
            return
        }
        
        if selectedIndex - 1 < 0 {
            setSelectedOpenDocById(maybeId: openDocumentsKeyArr.last)
        } else {
            setSelectedOpenDocById(maybeId: openDocumentsKeyArr[selectedIndex - 1])
        }
    }
    
    func formatSelectedDocSelectedText(_ textFormatting: TextFormatting) {
        if let id = selectedDoc {
            switch textFormatting {
            case .Heading(let headingSize):
                openDocuments[id]?.textDocumentToolbar?.toggleHeading(headingSize)
            case .Bold:
                openDocuments[id]?.textDocumentToolbar?.toggleBold()
            case .Italic:
                openDocuments[id]?.textDocumentToolbar?.toggleItalic()
            case .InlineCode:
                openDocuments[id]?.textDocumentToolbar?.toggleInlineCode()
            case .Strikethrough:
                openDocuments[id]?.textDocumentToolbar?.toggleStrikethrough()
            case .NumberList:
                openDocuments[id]?.textDocumentToolbar?.toggleNumberList()
            case .BulletList:
                openDocuments[id]?.textDocumentToolbar?.toggleBulletList()
            case .TodoList:
                openDocuments[id]?.textDocumentToolbar?.toggleTodoList()
            }
        }
    }
    
    func undoRedoSelectedDoc(redo: Bool) {
        if let id = selectedDoc {
            openDocuments[id]?.textDocumentToolbar?.undoRedo(redo)
        }
    }
}

public enum TextFormatting {
    case Heading(UInt32)
    case Bold
    case Italic
    case InlineCode
    case Strikethrough
    case NumberList
    case BulletList
    case TodoList
}

class DocumentLoadingInfo: ObservableObject {
    let core: LockbookApi
    let isiPhone: Bool
    
    @Published var meta: File
    @Published var type: ViewType
    @Published var deleted: Bool = false
    @Published var loading: Bool = true
    @Published var reloadContent: Bool = false
    @Published var error: String = ""

    @Published var textDocument: EditorState? = nil
    @Published var textDocumentToolbar: ToolbarState? = nil
    @Published var documentNameState: NameState = NameState()
    
    @Published var drawing: PKDrawing? = nil
    @Published var image: Image? = .none
    
    @Published var dismissForLink: File? = nil

    var timeCreated = Date()

    private var cancellables = Set<AnyCancellable>()

    init(_ meta: File, _ isiPhone: Bool) {
        self.core = DI.core
        self.meta = meta
        self.type = DocumentLoadingInfo.getType(name: meta.name)
        self.isiPhone = isiPhone
        
        drawingAutosaver()
    }
    
    func startLoading() {        
        if self.type == .Unknown {
            self.loading = false
            return
        }

        switch type {
        case .Markdown:
            loadMarkdown()
            #if os(iOS)
        case .Drawing:
            loadDrawing()
            #endif
        case .Image:
            loadImage()
        case .Unknown:
            self.loading = false
        }
    }

    func updatesFromCoreAvailable(_ newMeta: File) {
        self.meta = newMeta
        switch type {
        case .Markdown: // For markdown we're able to do a check before reloading the doc
            DispatchQueue.global(qos: .userInitiated).async {
                let operation = self.core.getFile(id: self.meta.id)
                DispatchQueue.main.async {
                    switch operation {
                    case .success(let txt):
                        if let editor = self.textDocument {
                            if editor.text != txt {
                                editor.reload = true
                                editor.text = txt
                            }
                        }
                    case .failure(let err):
                        print(err)
                    }
                }
            }
            #if os(iOS)
        case .Drawing:
            self.reloadContent = true
            loadDrawing()
            #endif
        case .Image:
            self.reloadContent = true
            loadImage()
        case .Unknown:
            print("cannot reload unknown content type")
        }

    }

    private func drawingAutosaver() {
        print("autosaver setup")
        $drawing
                .debounce(for: .milliseconds(100), scheduler: DispatchQueue.global(qos: .userInitiated))
                .sink(receiveValue: {
                    if let text = $0 { // TODO don't write if a reload or delete is required
                        self.writeDrawing(drawing: text)
                    }
                })
                .store(in: &cancellables)
    }

    private func loadMarkdown() {
        DispatchQueue.global(qos: .userInitiated).async {
            let operation = self.core.getFile(id: self.meta.id)

            DispatchQueue.main.async {
                switch operation {
                case .success(let txt):
                    self.textDocument = EditorState(text: txt, isiPhone: self.isiPhone)
                    self.textDocumentToolbar = ToolbarState()
                    self
                        .textDocument!
                        .$text
                        .debounce(for: .milliseconds(100), scheduler: DispatchQueue.global(qos: .userInitiated))
                        .sink(receiveValue: {
                            self.writeDocument(content: $0)
                        })
                        .store(in: &self.cancellables)
                case .failure(let err):
                    self.error = err.description
                }
                
                self.loading = false
            }
        }
    }

    private func writeDocument(content: String) {
        let operation = self.core.updateFile(id: meta.id, content: content)
        DispatchQueue.main.async {
            switch operation {
            case .success(_):
                DI.sync.documentChangeHappened()
            case .failure(let error):
                DI.errors.handleError(error)
            }
        }
    }

    private func loadImage() {
        DispatchQueue.global(qos: .userInitiated).async {
            let operation = self.core.exportDrawing(id: self.meta.id)

            DispatchQueue.main.async {
                switch operation {
                case .success(let data):
                    if let image = self.getImage(from: data) {
                        self.image = image
                    } else {
                        self.error = "Could not make NSImage from Data!"
                    }
                case .failure(let error):
                    self.error = error.description
                }
                self.loading = false
            }
        }
    }

    private func getImage(from: Data) -> Image? {
        #if os(macOS)
        if let nsImage = NSImage(data: from) {
            return Image(nsImage: nsImage)
        } else {
            return .none
        }
        #else
        if let uiImage = UIImage(data: from) {
            return Image(uiImage: uiImage)
        } else {
            return .none
        }
        #endif
    }

    private func loadDrawing() {
        DispatchQueue.global(qos: .userInitiated).async {
            let operation = self.core.readDrawing(id: self.meta.id)
            DispatchQueue.main.async {
                switch operation {
                case .success(let drawing):
                    self.drawing = PKDrawing(from: drawing)
                case .failure(let error):
                    self.error = error.description
                }
                self.loading = false
            }
        }
    }

    private func writeDrawing(drawing: PKDrawing) {
        switch self.core.writeDrawing(id: meta.id, content: Drawing(from: drawing)) {
        case .success(_):
            print("drawing saved successfully")
        case .failure(let error):
            DI.errors.handleError(error)
        }

        DI.sync.documentChangeHappened()
    }

    // TODO we need the swift clients to accept Data back as files, then we can read arbitary images
    private static func getType(name: String) -> ViewType {
        if name.lowercased().hasSuffix(".draw") {
            #if os(macOS)
            return .Image
            #else
            return .Drawing
            #endif
        } else if name.lowercased().hasSuffix(".md") || name.lowercased().hasSuffix(".markdown") || name.lowercased().hasSuffix(".txt") {
            return .Markdown
        } else {
            return .Unknown
        }
    }

}

public enum ViewType {
    case Markdown
    #if os(iOS)
    case Drawing
    #endif
    case Image
    case Unknown
}
