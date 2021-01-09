import Foundation
import SwiftLockbookCore
import SwiftUI
import Combine

class Core: ObservableObject {
    let documenstDirectory: String
    let api: LockbookApi
    @Published var state: DbState
    @Published var account: Account?
    @Published var globalError: AnyFfiError?
    @Published var files: [FileMetadata] = []
    @Published var root: FileMetadata?
    @Published var syncing: Bool = false {
        didSet {
            if oldValue == false && syncing == true {
                serialQueue.async {
                    self.passthrough.send(self.api.synchronize())
                }
            }
        }
    }
    let timer = Timer.publish(every: 30, on: .main, in: .common).autoconnect()
    let serialQueue = DispatchQueue(label: "syncQueue")
    
    private var passthrough = PassthroughSubject<FfiResult<SwiftLockbookCore.Empty, SyncAllError>, Never>()
    private var cancellableSet: Set<AnyCancellable> = []

    func load() {
        switch api.getAccount() {
        case .success(let acc):
            account = acc
        case .failure(let err):
            handleError(err)
        }
    }

    func migrate() {
        let res = api.migrateState()
            .eraseError()
            .flatMap(transform: { _ in api.getState().eraseError() })
        switch res {
        case .success(let newState):
            state = newState
            load()
            switch newState {
            case .ReadyToUse:
                break
            default:
                print("Weird state after migration: \(newState)")
            }
        case .failure(let err):
            handleError(err)
        }
    }

    func purge() {
        let lockbookDir = URL(fileURLWithPath: documenstDirectory).appendingPathComponent("lockbook.sled")
        if let _ = try? FileManager.default.removeItem(at: lockbookDir) {
            DispatchQueue.main.async {
                self.account = nil
                switch self.api.getState() {
                case .success(let db):
                    self.state = db
                case .failure(let err):
                    self.handleError(err)
                }
            }
        }
    }
    
    func sync() {
        self.syncing = true
        
    }
    
    func handleError(_ error: AnyFfiError) {
        globalError = error
    }
    
    private func buildTree(meta: FileMetadata) -> FileMetadataWithChildren {
        return FileMetadataWithChildren(meta: meta, children: files.filter({ $0.parent == meta.id && $0.id != meta.id }).map(buildTree))
    }
    
    func updateFiles() {
        if (account != nil) {
            switch api.getRoot() {
            case .success(let root):
                self.root = root
                switch api.listFiles() {
                case .success(let metas):
                    self.files = metas
                case .failure(let err):
                    handleError(err)
                }
            case .failure(let err):
                handleError(err)
            }
        }
    }
    
    init(documenstDirectory: String) {
        self.documenstDirectory = documenstDirectory
        self.api = CoreApi(documentsDirectory: documenstDirectory)
        self.state = (try? self.api.getState().get())!
        self.account = (try? self.api.getAccount().get())
        updateFiles()

        passthrough
            .debounce(for: .milliseconds(500), scheduler: RunLoop.main)
            .removeDuplicates(by: {
                switch ($0, $1) {
                case (.failure(let e1), .failure(let e2)):
                    return e1 == e2
                default:
                    return false
                }
            })
            .receive(on: RunLoop.main)
            .sink(receiveValue: { res in
                self.syncing = false
                switch res {
                case .success(_):
                    self.updateFiles()
                case .failure(let err):
                    self.handleError(err)
                }
            })
            .store(in: &cancellableSet)
    }
    
    init() {
        self.documenstDirectory = "<USING-FAKE-API>"
        self.api = FakeApi()
        self.state = .ReadyToUse
        self.account = Account(username: "testy", apiUrl: "ftp://lockbook.gov", keys: .empty)
        if case .success(let root) = api.getRoot(), case .success(let metas) = api.listFiles() {
            self.files = metas
            self.root = root
        }
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
