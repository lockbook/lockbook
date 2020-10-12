import Foundation
import CLockbookCore

public enum CoreResult<T: Decodable, E: Decodable & Equatable>: Decodable {
    case success(T)
    case failure(CoreError<E>)

    enum ResultKeys: String, CodingKey {
        case Ok
        case Err
    }

    func get() throws -> T {
        switch self {
        case .success(let t):
            return t
        case .failure(let err):
            throw err
        }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: ResultKeys.self)

        if (container.contains(.Ok)) {
            self = .success(try container.decode(T.self, forKey: .Ok))
        } else if (container.contains(.Err)) {
            self = .failure(try container.decode(CoreError<E>.self, forKey: .Err))
        } else {
            self = .failure(.Unexpected("Failed to deserialize \(String(describing: Self.self)): \(container.codingPath)"))
        }
    }
}

public protocol LockbookApi {
    // Account
    func getAccount() -> CoreResult<Account, GetAccountError>
    func createAccount(username: String, apiLocation: String) -> CoreResult<Empty, CreateAccountError>
    func importAccount(accountString: String) -> CoreResult<Empty, ImportError>
    func exportAccount() -> CoreResult<String, AccountExportError>
    func getUsage() -> CoreResult<[FileUsage], GetUsageError>
    
    // Work
    func synchronize() -> CoreResult<Empty, SyncAllError>
    func calculateWork() -> CoreResult<WorkMetadata, CalculateWorkError>
    func executeWork(work: WorkUnit) -> CoreResult<Empty, ExecuteWorkError>
    func setLastSynced(lastSync: UInt64) -> CoreResult<Empty, SetLastSyncedError>
    
    // Directory
    func getRoot() -> CoreResult<FileMetadata, GetRootError>
    func listFiles() -> CoreResult<[FileMetadata], ListMetadatasError>
    
    // Document
    func getFile(id: UUID) -> CoreResult<DecryptedValue, ReadDocumentError>
    func createFile(name: String, dirId: UUID, isFolder: Bool) -> CoreResult<FileMetadata, CreateFileError>
    func updateFile(id: UUID, content: String) -> CoreResult<Empty, WriteToDocumentError>
    func markFileForDeletion(id: UUID) -> CoreResult<Bool, DeleteFileError>
    func renameFile(id: UUID, name: String) -> CoreResult<Empty, RenameFileError>
}

public struct CoreApi: LockbookApi {
    var documentsDirectory: String
    
    public init(documentsDirectory: String) {
        self.documentsDirectory = documentsDirectory
        print("Located at \(documentsDirectory)")
    }
    
    /// If this isn't called, the rust logger will not start!
    public func initializeLogger() -> Void {
        init_logger_safely(documentsDirectory)
    }
    
    public func getAccount() -> CoreResult<Account, GetAccountError> {
        fromPrimitiveResult(result: get_account(documentsDirectory))
    }
    
    public func createAccount(username: String, apiLocation: String) -> CoreResult<Empty, CreateAccountError> {
        fromPrimitiveResult(result: create_account(documentsDirectory, username, apiLocation))
    }
    
    public func importAccount(accountString: String) -> CoreResult<Empty, ImportError> {
        fromPrimitiveResult(result: import_account(documentsDirectory, accountString.trimmingCharacters(in: .whitespacesAndNewlines)))
    }
    
    public func exportAccount() -> CoreResult<String, AccountExportError> {
        fromPrimitiveResult(result: export_account(documentsDirectory))
    }
    
    public func getUsage() -> CoreResult<[FileUsage], GetUsageError> {
        fromPrimitiveResult(result: get_usage(documentsDirectory))
    }
    
    public func synchronize() -> CoreResult<Empty, SyncAllError> {
        fromPrimitiveResult(result: sync_all(documentsDirectory))
    }
    
    public func calculateWork() -> CoreResult<WorkMetadata, CalculateWorkError> {
        fromPrimitiveResult(result: calculate_work(documentsDirectory))
    }
    
    public func executeWork(work: WorkUnit) -> CoreResult<Empty, ExecuteWorkError> {
        switch serialize(obj: work) {
        case .success(let str):
            return fromPrimitiveResult(result: execute_work(documentsDirectory, str))
        case .failure(let err):
            return .failure(.Unexpected(err.localizedDescription))
        }
    }
    
    public func setLastSynced(lastSync: UInt64) -> CoreResult<Empty, SetLastSyncedError> {
        fromPrimitiveResult(result: set_last_synced(documentsDirectory, lastSync))
    }
    
    public func getRoot() -> CoreResult<FileMetadata, GetRootError> {
        fromPrimitiveResult(result: get_root(documentsDirectory))
    }
    
    public func listFiles() -> CoreResult<[FileMetadata], ListMetadatasError> {
        fromPrimitiveResult(result: list_metadatas(documentsDirectory))
    }
    
    public func getFile(id: UUID) -> CoreResult<DecryptedValue, ReadDocumentError> {
        fromPrimitiveResult(result: read_document(documentsDirectory, id.uuidString))
    }
    
    public func createFile(name: String, dirId: UUID, isFolder: Bool) -> CoreResult<FileMetadata, CreateFileError> {
        let fileType = isFolder ? "Folder" : "Document"
        return fromPrimitiveResult(result: create_file(documentsDirectory, name, dirId.uuidString, fileType))
    }
    
    public func updateFile(id: UUID, content: String) -> CoreResult<Empty, WriteToDocumentError> {
        fromPrimitiveResult(result: write_document(documentsDirectory, id.uuidString, content))
    }
    
    public func markFileForDeletion(id: UUID) -> CoreResult<Bool, DeleteFileError> {
        CoreResult.failure(CoreError.lazy())
    }
    
    public func renameFile(id: UUID, name: String) -> CoreResult<Empty, RenameFileError> {
        fromPrimitiveResult(result: rename_file(documentsDirectory, id.uuidString, name))
    }
}
