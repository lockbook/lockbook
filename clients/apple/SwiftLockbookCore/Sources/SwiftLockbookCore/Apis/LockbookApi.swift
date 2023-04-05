import Foundation

public protocol LockbookApi {
    // Account
    func getAccount() -> FfiResult<Account, GetAccountError>
    func createAccount(username: String, apiLocation: String, welcomeDoc: Bool) -> FfiResult<Empty, CreateAccountError>
    func importAccount(accountString: String) -> FfiResult<Empty, ImportError>
    func exportAccount() -> FfiResult<String, AccountExportError>
    func getUsage() -> FfiResult<UsageMetrics, GetUsageError>
    func getUncompressedUsage() -> FfiResult<UsageItemMetric, GetUsageError>
    func deleteAccount() -> FfiResult<Empty, DeleteAccountError>

    // Work
    func syncAll() -> FfiResult<Empty, SyncAllError>
    func calculateWork() -> FfiResult<WorkCalculated, CalculateWorkError>
    func getLastSyncedHumanString() -> FfiResult<String, GetLastSyncedError>
    func getLocalChanges() -> FfiResult<[UUID], GetLocalChangesError>
    
    // Directory
    func getRoot() -> FfiResult<File, GetRootError>
    func listFiles() -> FfiResult<[File], ListMetadatasError>
    
    // Document
    func getFile(id: UUID) -> FfiResult<String, ReadDocumentError>
    func createFile(name: String, dirId: UUID, isFolder: Bool) -> FfiResult<File, CreateFileError>
    func createLink(name: String, dirId: UUID, target: UUID) -> FfiResult<Empty, CreateFileError>
    func updateFile(id: UUID, content: String) -> FfiResult<Empty, WriteToDocumentError>
    func deleteFile(id: UUID) -> FfiResult<Empty, FileDeleteError>
    func renameFile(id: UUID, name: String) -> FfiResult<Empty, RenameFileError>
    func moveFile(id: UUID, newParent: UUID) -> FfiResult<Empty, MoveFileError>
    func readDrawing(id: UUID) -> FfiResult<Drawing, GetDrawingError>
    func writeDrawing(id: UUID, content: Drawing) -> FfiResult<Empty, WriteToDocumentError>
    func exportDrawing(id: UUID) -> FfiResult<Data, ExportDrawingError>
    func shareFile(id: UUID, username: String, isWrite: Bool) -> FfiResult<Empty, ShareFileError>
    func getPendingShares() -> FfiResult<[File], GetPendingShares>
    func deletePendingShare(id: UUID) ->FfiResult<Empty, DeletePendingShareError>
    func exportFile(id: UUID, destination: String) ->FfiResult<Empty, ExportFileError>
    func importFiles(sources: [String], destination: UUID) ->FfiResult<Empty, ImportFilesError>
    func getFileById(id: UUID) -> FfiResult<File, GetFileByIdError>
    
    // Billing
    func newAppleSub(originalTransactionId: String, appAccountToken: String) -> FfiResult<Empty, UpgradeAccountAppStoreError>
    func cancelSub() -> FfiResult<Empty, CancelSubscriptionError>
    
    // Search
    func searchFilePaths(input: String) ->FfiResult<[SearchResultItem], SearchFilePathsError>
    func startSearch(context: UnsafeRawPointer?, updateStatus: @convention(c) (UnsafePointer<Int8>?, Int32, UnsafePointer<Int8>?) -> Void) -> FfiResult<Empty, GeneralSearchError>
    func endSearch() -> FfiResult<Empty, GeneralSearchError>
    func searchQuery(query: String) -> FfiResult<Empty, GeneralSearchError>
    func stopCurrentSearch() -> FfiResult<Empty, GeneralSearchError>
}
