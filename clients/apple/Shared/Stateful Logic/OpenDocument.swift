import SwiftUI
import SwiftLockbookCore
import Combine

class OpenDocument: ObservableObject {
    let core: LockbookApi
    
    @Published var loadText: String?
    @Published var reloadText: Bool = false
    @Published var saveText: String?

    @Published var meta: ClientFileMetadata?
    @Published var deleted: Bool = false
    var cancellables = Set<AnyCancellable>()
    @Published var status: Status = .Inactive

    init(_ core: LockbookApi) {
        self.core = core
        
        $saveText
            .debounce(for: .seconds(1), scheduler: DispatchQueue.global(qos: .userInitiated))
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
        let operation = self.core.updateFile(id: meta.id, content: content)
        DispatchQueue.main.async {
            switch operation {
            case .success(_):
                DI.sync.documentChangeHappened()
                withAnimation {
                    self.status = .WriteSuccess
                }
            case .failure(let err):
                print(err)
            }
        }
    }
    
    func openDocument(meta: ClientFileMetadata) {
        self.deleted = false
        DispatchQueue.main.async {
            switch self.core.getFile(id: meta.id) {
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
        switch self.core.getFile(id: meta.id) {
        case .success(let txt):
            if self.saveText != txt {
                self.meta = meta
                self.loadText = txt
                self.reloadText = true
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
