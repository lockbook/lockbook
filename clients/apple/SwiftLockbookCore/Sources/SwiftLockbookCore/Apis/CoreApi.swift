import Foundation
import CLockbookCore

public typealias CoreResult<T> = Result<T, ApplicationError>

public protocol LockbookApi {
    // Account
    func getAccount() -> CoreResult<Account>
    func createAccount(username: String, apiLocation: String) -> CoreResult<Account>
    func importAccount(accountString: String) -> CoreResult<Account>
    func exportAccount() -> CoreResult<String>
    func getUsage() -> CoreResult<[FileUsage]>
    
    // Work
    func synchronize() -> CoreResult<Empty>
    func calculateWork() -> CoreResult<WorkMetadata>
    func executeWork(work: WorkUnit) -> CoreResult<Empty>
    func setLastSynced(lastSync: UInt64) -> CoreResult<Empty>
    
    // Directory
    func getRoot() -> CoreResult<FileMetadata>
    func listFiles() -> CoreResult<[FileMetadata]>
    
    // Document
    func getFile(id: UUID) -> CoreResult<DecryptedValue>
    func createFile(name: String, dirId: UUID, isFolder: Bool) -> CoreResult<FileMetadata>
    func updateFile(id: UUID, content: String) -> CoreResult<Empty>
    func markFileForDeletion(id: UUID) -> CoreResult<Bool>
    func renameFile(id: UUID, name: String) -> CoreResult<Empty>
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
    
    public func getAccount() -> CoreResult<Account> {
        fromPrimitiveResult(result: get_account(documentsDirectory))
    }
    
    public func createAccount(username: String, apiLocation: String) -> CoreResult<Account> {
        let result: Result<Empty, ApplicationError> = fromPrimitiveResult(result: create_account(documentsDirectory, username, apiLocation))
        
        return result.flatMap { print($0 as Any); return getAccount() }
    }
    
    public func importAccount(accountString: String) -> CoreResult<Account> {
        let result: Result<Empty, ApplicationError> = fromPrimitiveResult(result: import_account(documentsDirectory, accountString.trimmingCharacters(in: .whitespacesAndNewlines)))
        
        return result.flatMap { print($0 as Any); return getAccount() }
    }
    
    public func exportAccount() -> CoreResult<String> {
        fromPrimitiveResult(result: export_account(documentsDirectory))
    }
    
    public func getUsage() -> CoreResult<[FileUsage]> {
        fromPrimitiveResult(result: get_usage(documentsDirectory))
    }
    
    public func synchronize() -> CoreResult<Empty> {
        fromPrimitiveResult(result: sync_all(documentsDirectory))
    }
    
    public func calculateWork() -> CoreResult<WorkMetadata> {
        fromPrimitiveResult(result: calculate_work(documentsDirectory))
    }
    
    public func executeWork(work: WorkUnit) -> CoreResult<Empty> {
        serialize(obj: work).flatMap({
            fromPrimitiveResult(result: execute_work(documentsDirectory, $0))
        })
    }
    
    public func setLastSynced(lastSync: UInt64) -> CoreResult<Empty> {
        fromPrimitiveResult(result: set_last_synced(documentsDirectory, lastSync))
    }
    
    public func getRoot() -> CoreResult<FileMetadata> {
        fromPrimitiveResult(result: get_root(documentsDirectory))
    }
    
    public func listFiles() -> CoreResult<[FileMetadata]> {
        fromPrimitiveResult(result: list_metadatas(documentsDirectory))
    }
    
    public func getFile(id: UUID) -> CoreResult<DecryptedValue> {
        fromPrimitiveResult(result: read_document(documentsDirectory, id.uuidString))
    }
    
    public func createFile(name: String, dirId: UUID, isFolder: Bool) -> CoreResult<FileMetadata> {
        let fileType = isFolder ? "Folder" : "Document"
        return fromPrimitiveResult(result: create_file(documentsDirectory, name, dirId.uuidString, fileType))
    }
    
    public func updateFile(id: UUID, content: String) -> CoreResult<Empty> {
        fromPrimitiveResult(result: write_document(documentsDirectory, id.uuidString, content))
    }
    
    public func markFileForDeletion(id: UUID) -> CoreResult<Bool> {
        CoreResult.failure(ApplicationError.lazy())
    }
    
    public func renameFile(id: UUID, name: String) -> CoreResult<Empty> {
        fromPrimitiveResult(result: rename_file(documentsDirectory, id.uuidString, name))
    }
}
