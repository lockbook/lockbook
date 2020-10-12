import Foundation
import CLockbookCore

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
    
    public func getAccount() -> FfiResult<Account, GetAccountError> {
        fromPrimitiveResult(result: get_account(documentsDirectory))
    }
    
    public func createAccount(username: String, apiLocation: String) -> FfiResult<Empty, CreateAccountError> {
        fromPrimitiveResult(result: create_account(documentsDirectory, username, apiLocation))
    }
    
    public func importAccount(accountString: String) -> FfiResult<Empty, ImportError> {
        fromPrimitiveResult(result: import_account(documentsDirectory, accountString.trimmingCharacters(in: .whitespacesAndNewlines)))
    }
    
    public func exportAccount() -> FfiResult<String, AccountExportError> {
        fromPrimitiveResult(result: export_account(documentsDirectory))
    }
    
    public func getUsage() -> FfiResult<[FileUsage], GetUsageError> {
        fromPrimitiveResult(result: get_usage(documentsDirectory))
    }
    
    public func synchronize() -> FfiResult<Empty, SyncAllError> {
        fromPrimitiveResult(result: sync_all(documentsDirectory))
    }
    
    public func calculateWork() -> FfiResult<WorkMetadata, CalculateWorkError> {
        fromPrimitiveResult(result: calculate_work(documentsDirectory))
    }
    
    public func executeWork(work: WorkUnit) -> FfiResult<Empty, ExecuteWorkError> {
        switch serialize(obj: work) {
        case .success(let str):
            return fromPrimitiveResult(result: execute_work(documentsDirectory, str))
        case .failure(let err):
            return .failure(.init(unexpected: err.localizedDescription))
        }
    }
    
    public func setLastSynced(lastSync: UInt64) -> FfiResult<Empty, SetLastSyncedError> {
        fromPrimitiveResult(result: set_last_synced(documentsDirectory, lastSync))
    }
    
    public func getRoot() -> FfiResult<FileMetadata, GetRootError> {
        fromPrimitiveResult(result: get_root(documentsDirectory))
    }
    
    public func listFiles() -> FfiResult<[FileMetadata], ListMetadatasError> {
        fromPrimitiveResult(result: list_metadatas(documentsDirectory))
    }
    
    public func getFile(id: UUID) -> FfiResult<DecryptedValue, ReadDocumentError> {
        fromPrimitiveResult(result: read_document(documentsDirectory, id.uuidString))
    }
    
    public func createFile(name: String, dirId: UUID, isFolder: Bool) -> FfiResult<FileMetadata, CreateFileError> {
        let fileType = isFolder ? "Folder" : "Document"
        return fromPrimitiveResult(result: create_file(documentsDirectory, name, dirId.uuidString, fileType))
    }
    
    public func updateFile(id: UUID, content: String) -> FfiResult<Empty, WriteToDocumentError> {
        fromPrimitiveResult(result: write_document(documentsDirectory, id.uuidString, content))
    }
    
    public func markFileForDeletion(id: UUID) -> FfiResult<Bool, DeleteFileError> {
        FfiResult.failure(.init(unexpected: "Bunk"))
    }
    
    public func renameFile(id: UUID, name: String) -> FfiResult<Empty, RenameFileError> {
        fromPrimitiveResult(result: rename_file(documentsDirectory, id.uuidString, name))
    }

    public func getState() -> FfiResult<DbState, GetStateError> {
        fromPrimitiveResult(result: get_db_state(documentsDirectory))
    }

    public func migrateState() -> FfiResult<Empty, MigrationError> {
        fromPrimitiveResult(result: migrate_db(documentsDirectory))
    }
}
