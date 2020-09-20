import Foundation
import SwiftLockbookCore
import SwiftUI
import Combine

class Core: ObservableObject {
    let documenstDirectory: String
    let api: LockbookApi
    @Published var account: Account?
    @Published var message: Message? = nil
    @Published var files: [FileMetadata] = []
    @Published var grouped: [FileMetadataWithChildren] = []

    private var cancellableSet: Set<AnyCancellable> = []
    @Published var editStream: (UUID, String)?
    @Published var currentEdits: [UUID: String] = [:]
    @Published var saver: SaveStatus = .Inactive
    
    func purge() {
        let lockbookDir = URL(fileURLWithPath: documenstDirectory).appendingPathComponent("lockbook.sled")
        if let _ = try? FileManager.default.removeItem(at: lockbookDir) {
            print("Deleted \(lockbookDir) and logging out")
            self.account = nil
        }
    }
    
    func sync() {
        switch api.synchronize() {
        case .success(_):
            updateFiles()
        case .failure(let err):
            displayError(error: err)
        }
    }
    
    func displayError(error: ApplicationError) {
        switch error {
        case .Lockbook(let err):
            self.message = Message(words: err.localizedDescription, icon: "xmark.shield.fill", color: .yellow)
        case .Serialization(let msg):
            self.message = Message(words: msg, icon: "square.fill.and.line.vertical.square.fill", color: .purple)
        case .State(let msg):
            self.message = Message(words: msg, icon: "burst.fill", color: .red)
        case .General(let err):
            self.message = Message(words: err.localizedDescription, icon: "exclamationmark.square.fill", color: .red)
        }
    }
    
    private func buildTree(meta: FileMetadata) -> FileMetadataWithChildren {
        return FileMetadataWithChildren(meta: meta, children: files.filter({ $0.parent == meta.id && $0.id != meta.id }).map(buildTree))
    }
    
    func updateFiles() {
        switch api.getRoot() {
        case .success(let root):
            switch api.listFiles() {
            case .success(let metas):
                self.files = metas
                self.grouped = buildTree(meta: root).children ?? []
            case .failure(let err):
                displayError(error: err)
            }
        case .failure(let err):
            displayError(error: err)
        }
    }
    
    init(documenstDirectory: String) {
        self.documenstDirectory = documenstDirectory
        let api = CoreApi(documentsDirectory: documenstDirectory)
        api.initializeLogger()
        switch api.getAccount() {
        case .success(let acc):
            self.account = acc
        case .failure(let err):
            print(err)
        }
        self.api = api
        self.updateFiles()
        
        $editStream
            .sink(receiveValue: {
                $0.map { (id, content) in
                    self.currentEdits[id] = content
                }
            })
            .store(in: &cancellableSet)
        
        $currentEdits
            .debounce(for: 5, scheduler: RunLoop.main)
            .sink(receiveValue: {
                $0.map({ update in
                    print("Trying to write \(update.key) \(update.value.count) chars")
                    switch api.updateFile(id: update.key, content: update.value) {
                    case .success(_):
                        self.saver = .Succeeded
                    case .failure(let err):
                        self.saver = .Failed
                        self.displayError(error: err)
                    }
                })
            })
            .store(in: &cancellableSet)
    }
    
    init() {
        self.documenstDirectory = "<USING-FAKE-API>"
        self.api = FakeApi()
        self.updateFiles()
    }
}

struct FileMetadataWithChildren: Identifiable {
    let id: UUID
    let meta: FileMetadata
    let children: [FileMetadataWithChildren]?
    
    init(meta: FileMetadata, children: [FileMetadataWithChildren]) {
        self.id = meta.id
        self.meta = meta
        if !children.isEmpty {
            self.children = children
        } else {
            self.children = nil
        }
    }
}

struct Message {
    let words: String
    let icon: String?
    let color: Color
}

enum SaveStatus {
    case Succeeded
    case Failed
    case Inactive
}
