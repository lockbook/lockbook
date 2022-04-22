import Foundation

public protocol LockbookApi {
    // Account
    func getAccount() -> FfiResult<Account, GetAccountError>
    func createAccount(username: String, apiLocation: String) -> FfiResult<Empty, CreateAccountError>
    func importAccount(accountString: String) -> FfiResult<Empty, ImportError>
    func exportAccount() -> FfiResult<String, AccountExportError>
    func getUsage() -> FfiResult<UsageMetrics, GetUsageError>
    func getUncompressedUsage() -> FfiResult<UsageItemMetric, GetUsageError>

    // Work
    func syncAll() -> FfiResult<Empty, SyncAllError>
    func calculateWork() -> FfiResult<WorkMetadata, CalculateWorkError>
    func getLastSyncedHumanString() -> FfiResult<String, GetLastSyncedError>
    func getLocalChanges() -> FfiResult<[UUID], GetLocalChangesError>
    
    // Directory
    func getRoot() -> FfiResult<DecryptedFileMetadata, GetRootError>
    func listFiles() -> FfiResult<[DecryptedFileMetadata], ListMetadatasError>
    
    // Document
    func getFile(id: UUID) -> FfiResult<String, ReadDocumentError>
    func createFile(name: String, dirId: UUID, isFolder: Bool) -> FfiResult<DecryptedFileMetadata, CreateFileError>
    func updateFile(id: UUID, content: String) -> FfiResult<Empty, WriteToDocumentError>
    func deleteFile(id: UUID) -> FfiResult<Empty, FileDeleteError>
    func renameFile(id: UUID, name: String) -> FfiResult<Empty, RenameFileError>
    func moveFile(id: UUID, newParent: UUID) -> FfiResult<Empty, MoveFileError>
    func readDrawing(id: UUID) -> FfiResult<Drawing, GetDrawingError>
    func writeDrawing(id: UUID, content: Drawing) -> FfiResult<Empty, WriteToDocumentError>
    func exportDrawing(id: UUID) -> FfiResult<Data, ExportDrawingError>
}
