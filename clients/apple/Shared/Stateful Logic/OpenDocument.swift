import SwiftUI
import SwiftLockbookCore
import Combine

class Content: ObservableObject {
    @Published var loadText: String?
    @Published var saveText: String?

    @Published var meta: ClientFileMetadata?
    @Published var deleted: Bool = false
    var cancellables = Set<AnyCancellable>()
    @Published var status: Status = .Inactive
    let write: (UUID, String) -> FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError>
    let read: (UUID) -> FfiResult<String, ReadDocumentError>
    
    init(write: @escaping (UUID, String) -> FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError>, read: @escaping (UUID) -> FfiResult<String, ReadDocumentError>) {
        self.read = read
        self.write = write
        
        $saveText
            .debounce(for: .seconds(1), scheduler: DispatchQueue.main)
            .sink(receiveValue: {
                if let c = $0, let m = self.meta {
                    self.writeDocument(meta: m, content: c)
                }
            })
            .store(in: &cancellables)
    }
    
    func updateText(text: String) {
        self.saveText = text
        status = .Inactive
    }
    
    func writeDocument(meta: ClientFileMetadata, content: String) {
        switch write(meta.id, content) {
        case .success(_):
            DI.sync.documentChangeHappened()
            withAnimation {
                status = .WriteSuccess
            }
        case .failure(let err):
            print(err)
        }
    }
    
    func openDocument(meta: ClientFileMetadata) {
        self.deleted = false
        DispatchQueue.main.async {
            switch self.read(meta.id) {
            case .success(let txt):
                self.meta = meta
                self.loadText = txt
                self.saveText = txt
            case .failure(let err):
                print(err)
            }
        }
    }
    
    func closeDocument() {
        meta = .none
        loadText = .none
        saveText = .none
    }
    
    func reloadDocumentIfNeeded(meta: ClientFileMetadata) {
        switch self.read(meta.id) {
        case .success(let txt):
            if self.saveText != txt {
                self.closeDocument()
                self.meta = meta
                self.loadText = txt
                self.saveText = txt
            }
        case .failure(let err):
            print(err)
        }
        
    }
}

enum Status {
    case WriteSuccess
    case WriteFailure
    case Inactive
}
