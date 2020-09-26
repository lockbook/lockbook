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
    @Published var syncing: Bool = false
    let timer = Timer.publish(every: 30, on: .main, in: .common).autoconnect()
    
    private var passthrough = PassthroughSubject<Void, ApplicationError>()
    private var cancellableSet: Set<AnyCancellable> = []
    
    func purge() {
        let lockbookDir = URL(fileURLWithPath: documenstDirectory).appendingPathComponent("lockbook.sled")
        if let _ = try? FileManager.default.removeItem(at: lockbookDir) {
            print("Deleted \(lockbookDir) and logging out")
            self.account = nil
        }
    }
    
    func sync() {
        self.syncing = true
        DispatchQueue.global(qos: .background).async {
            switch self.api.synchronize() {
            case .success(_):
                self.passthrough.send(())
            case .failure(let err):
                self.passthrough.send(completion: .failure(err))
            }
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
                self.grouped = [buildTree(meta: root)]
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
        print("API URL: \(api.getApiLocation())")
        self.updateFiles()
        
        passthrough
            .receive(on: RunLoop.main)
            .sink(receiveCompletion: { err in
                self.syncing = false
                switch err {
                case .failure(let err):
                    print("Error \(err.message())")
                    self.displayError(error: err)
                case .finished:
                    print("Sync subscription finished!")
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
