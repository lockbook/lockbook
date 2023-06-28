import Foundation
import SwiftLockbookCore
import Combine
import PencilKit
import SwiftUI
import SwiftEditor

public enum ViewType {
    case Markdown
    #if os(iOS)
    case Drawing
    #endif
    case Image
    case Unknown
}

class DocumentLoader: ObservableObject {
    
    let core: LockbookApi
    
    @Published var meta: File?
    @Published var type: ViewType?
    @Published var deleted: Bool = false
    @Published var loading: Bool = true
    @Published var reloadContent: Bool = false
    @Published var error: String = ""

    @Published var textDocument: EditorState?
    @Published var drawing: PKDrawing?
    @Published var image: Image? = .none

    private var cancellables = Set<AnyCancellable>()

    init(_ core: LockbookApi) {
        self.core = core
        drawingAutosaver()
    }

    func startLoading(_ meta: File) {
        let type = DocumentLoader.getType(name: meta.name)

        self.meta = meta
        self.type = type
        self.deleted = false
        self.loading = true
        self.reloadContent = false
        self.textDocument = nil
        self.drawing = nil
        self.image = nil
        self.error = ""

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
        if
                let type = self.type,
                let meta = self.meta {
            switch type {
            case .Markdown: // For markdown we're able to do a check before reloading the doc
                DispatchQueue.global(qos: .userInitiated).async {
                    let operation = self.core.getFile(id: meta.id)
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
        } else {
            print("should not be reached")
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
        if let meta = self.meta {
            DispatchQueue.global(qos: .userInitiated).async {
                let operation = self.core.getFile(id: meta.id)

                DispatchQueue.main.async {
                    switch operation {
                    case .success(let txt):
                        self.textDocument = EditorState(text: txt, name: meta.name.replacingOccurrences(of: ".md", with: ""))
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
    }

    private func writeDocument(content: String) {
        if let meta = self.meta {
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
    }

    private func loadImage() {
        if let meta = self.meta {

            DispatchQueue.global(qos: .userInitiated).async {
                let operation = self.core.exportDrawing(id: meta.id)

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
        if let meta = self.meta {
            DispatchQueue.global(qos: .userInitiated).async {
                let operation = self.core.readDrawing(id: meta.id)
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

    }

    private func writeDrawing(drawing: PKDrawing) {
        if let meta = self.meta {

            switch self.core.writeDrawing(id: meta.id, content: Drawing(from: drawing)) {
            case .success(_):
                print("drawing saved successfully")
            case .failure(let error):
                DI.errors.handleError(error)
            }

            DI.sync.documentChangeHappened()
        }
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
