import SwiftUI
import SwiftLockbookCore
import Combine

class Content: ObservableObject {
    @Published var text: String?
    @Published var meta: ClientFileMetadata?
    var cancellables = Set<AnyCancellable>()
    @Published var status: Status = .Inactive
    let write: (UUID, String) -> FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError>
    let read: (UUID) -> FfiResult<String, ReadDocumentError>
    
    init(write: @escaping (UUID, String) -> FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError>, read: @escaping (UUID) -> FfiResult<String, ReadDocumentError>) {
        self.read = read
        self.write = write
        
        $text
            .debounce(for: .seconds(1), scheduler: DispatchQueue.main)
            .sink(receiveValue: {
                if let c = $0, let m = self.meta {
                    self.writeDocument(meta: m, content: c)
                }
            })
            .store(in: &cancellables)
    }
    
    func updateText(text: String) {
        self.text = text
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
        print("open document called")
        DispatchQueue.main.async {
            switch self.read(meta.id) {
            case .success(let txt):
                self.meta = meta
                self.text = txt
            case .failure(let err):
                print(err)
            }
        }
    }
    
    func closeDocument() {
        print("Close document")
        meta = .none
        text = .none
    }
    
    func reloadDocumentIfNeeded(meta: ClientFileMetadata) {
        print("reload document called")
        switch self.read(meta.id) {
        case .success(let txt):
            if self.text != txt { /// Close the document
                print("reload")
                self.closeDocument()
                self.meta = meta
                self.text = txt
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
