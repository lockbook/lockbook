import Foundation

public protocol LockbookApi {
    // Account
    func getAccount() -> FfiResult<Account, GetAccountError>
    func createAccount(username: String, apiLocation: String) -> FfiResult<Empty, CreateAccountError>
    func importAccount(accountString: String) -> FfiResult<Empty, ImportError>
    func exportAccount() -> FfiResult<String, AccountExportError>
    func getUsage() -> FfiResult<[FileUsage], GetUsageError>
    func getUsageHumanReadable() -> FfiResult<String, GetUsageError>
    
    // Work
    func syncAll() -> FfiResult<Empty, SyncAllError>
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
    func deleteFile(id: UUID) -> FfiResult<Empty, FileDeleteError>
    func renameFile(id: UUID, name: String) -> FfiResult<Empty, RenameFileError>
    func moveFile(id: UUID, newParent: UUID) -> FfiResult<Empty, MoveFileError>
    func readDrawing(id: UUID) -> FfiResult<Drawing, ReadDocumentError>
    func writeDrawing(id: UUID, content: Drawing) -> FfiResult<Empty, WriteToDocumentError>

    // State
    func getState() -> FfiResult<DbState, GetStateError>
    func migrateState() -> FfiResult<Empty, MigrationError>
}
