import Foundation

public protocol LockbookApi {
    // Account
    func getAccount() -> FfiResult<Account, GetAccountError>
    func createAccount(username: String, apiLocation: String) -> FfiResult<Empty, CreateAccountError>
    func importAccount(accountString: String) -> FfiResult<Empty, ImportError>
    func exportAccount() -> FfiResult<String, AccountExportError>
    func getUsage() -> FfiResult<[FileUsage], GetUsageError>
    
    // Work
    func synchronize() -> FfiResult<Empty, SyncAllError>
    func calculateWork() -> FfiResult<WorkMetadata, CalculateWorkError>
    func executeWork(work: WorkUnit) -> FfiResult<Empty, ExecuteWorkError>
    func setLastSynced(lastSync: UInt64) -> FfiResult<Empty, SetLastSyncedError>
    
    // Directory
    func getRoot() -> FfiResult<FileMetadata, GetRootError>
    func listFiles() -> FfiResult<[FileMetadata], ListMetadatasError>
    
    // Document
    func getFile(id: UUID) -> FfiResult<String, ReadDocumentError>
    func createFile(name: String, dirId: UUID, isFolder: Bool) -> FfiResult<FileMetadata, CreateFileError>
    func updateFile(id: UUID, content: String) -> FfiResult<Empty, WriteToDocumentError>
    func markFileForDeletion(id: UUID) -> FfiResult<Bool, DeleteFileError>
    func renameFile(id: UUID, name: String) -> FfiResult<Empty, RenameFileError>

    // State
    func getState() -> FfiResult<DbState, GetStateError>
    func migrateState() -> FfiResult<Empty, MigrationError>
}
