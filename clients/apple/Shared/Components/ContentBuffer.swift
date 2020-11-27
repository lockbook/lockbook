import Foundation
import Combine
import SwiftLockbookCore

class ContentBuffer: ObservableObject {
    let meta: FileMetadata
    private var cancellables: Set<AnyCancellable> = []
    let core: Core
    @Published var content: String
    @Published var succeeded: Bool = false
    @Published var status: Status = .Inactive
    @Published var title: String

    init(meta: FileMetadata, initialContent: String, core: Core) {
        self.meta = meta
        self.core = core
        self.content = initialContent
        self.title = meta.name

        $content
            .debounce(for: 0.2, scheduler: RunLoop.main)
            .sink { _ in
                self.status = .Inactive
            }
            .store(in: &cancellables)

        $content
            .debounce(for: 1, scheduler: DispatchQueue.global(qos: .background))
            .filter({ _ in self.succeeded })
            .flatMap { _ in
                Future<FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError>, Never> { promise in
                    promise(.success(self.save()))
                }
            }
            .eraseToAnyPublisher()
            .receive(on: RunLoop.main)
            .sink(receiveCompletion: { (err) in
                self.succeeded = false
                self.status = .BufferDied
            }, receiveValue: { (input) in
                switch input {
                case .success(_):
                    self.succeeded = true
                    self.status = .WriteSuccess
                case .failure(let err):
                    core.handleError(err)
                    self.status = .WriteFailure
                }

            })
            .store(in: &cancellables)
    }

    func save() -> FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError> {
        core.serialQueue.sync {
            switch core.api.updateFile(id: meta.id, content: content) {
            case .success(let e):
                return .success(e)
            case .failure(let err):
                return .failure(err)
            }
        }
    }

    enum Status {
        case BufferDied
        case WriteSuccess
        case WriteFailure
        case RenameSuccess
        case RenameFailure
        case Inactive
    }
}
