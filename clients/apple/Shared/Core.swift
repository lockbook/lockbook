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
    @Published var grouped: [FileMetadataWithChildren] = []
    @Published var syncing: Bool = false
    let timer = Timer.publish(every: 30, on: .main, in: .common).autoconnect()
    let serialQueue = DispatchQueue(label: "syncQueue")
    
    private var passthrough = PassthroughSubject<Void, Error>()
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
            }
        }
    }
    
    func sync() {
        self.syncing = true
        serialQueue.async {
            switch self.api.synchronize() {
            case .success(_):
                self.passthrough.send(())
            case .failure(let err):
                self.passthrough.send(completion: .failure(err))
            }
        }
    }
    
    func handleError<E: Error>(_ error: E) {
        switch error {
        case let ffiError as AnyFfiError:
            globalError = ffiError
        default:
            print("Received non-FFI error [\(String(describing: error.self))] \(error)") // This is basically an app crash
        }
    }
    
    private func buildTree(meta: FileMetadata) -> FileMetadataWithChildren {
        return FileMetadataWithChildren(meta: meta, children: files.filter({ $0.parent == meta.id && $0.id != meta.id }).map(buildTree))
    }
    
    func updateFiles() {
        if (account != nil) {
            switch api.getRoot() {
            case .success(let root):
                switch api.listFiles() {
                case .success(let metas):
                    self.files = metas
                    self.grouped = [buildTree(meta: root)]
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
            .receive(on: RunLoop.main)
            .sink(receiveCompletion: { err in
                self.syncing = false
                switch err {
                case .failure(let err):
                    self.handleError(err)
                case .finished:
                    print("Sync subscription finished!") // TODO: Does the application work at this point?
                }
            }, receiveValue: { _ in
                self.updateFiles()
                self.syncing = false
            })
            .store(in: &cancellableSet)
    }
    
    init() {
        self.documenstDirectory = "<USING-FAKE-API>"
        self.api = FakeApi()
        self.state = .ReadyToUse
        if case .success(let root) = api.getRoot(), case .success(let metas) = api.listFiles() {
            self.files = metas
            self.grouped = [buildTree(meta: root)]
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
