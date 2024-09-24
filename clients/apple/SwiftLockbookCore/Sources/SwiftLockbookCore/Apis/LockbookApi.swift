import Foundation

public protocol LockbookApi {
    // Account
    func getAccount() -> FfiResult<Account, GetAccountError>
    func createAccount(username: String, apiLocation: String, welcomeDoc: Bool) -> FfiResult<Empty, CreateAccountError>
    func importAccount(accountString: String) -> FfiResult<Empty, ImportError>
    func exportAccount() -> FfiResult<String, AccountExportError>
    func exportAccountPhrase() -> FfiResult<String, AccountExportError>
    func getUsage() -> FfiResult<UsageMetrics, GetUsageError>
    func getUncompressedUsage() -> FfiResult<UsageItemMetric, GetUsageError>
    func deleteAccount() -> FfiResult<Empty, DeleteAccountError>
    func logoutAndExit()

    // Work
    func syncAll(
        context: UnsafeRawPointer?,
        // updateStatus(context, isPushing, fileName)
        updateStatus: @escaping @convention(c) (UnsafePointer<Int8>?, UnsafePointer<Int8>?, Float) -> Void
    ) -> FfiResult<Empty, SyncAllError>
    func backgroundSync() -> FfiResult<Empty, SyncAllError>
    func calculateWork() -> FfiResult<SyncStatus, CalculateWorkError>
    func getLastSyncedHumanString() -> FfiResult<String, GetLastSyncedError>
    func getLocalChanges() -> FfiResult<[UUID], GetLocalChangesError>
    
    // Directory
    func getRoot() -> FfiResult<File, GetRootError>
    func listFiles() -> FfiResult<[File], ListMetadatasError>
    func listFolderPaths() -> FfiResult<[String], ListPathsError>
    
    // Document
    func readDocument(id: UUID) -> FfiResult<String, ReadDocumentError>
    func createFile(name: String, dirId: UUID, isFolder: Bool) -> FfiResult<File, CreateFileError>
    func createLink(name: String, dirId: UUID, target: UUID) -> FfiResult<Empty, CreateFileError>
    func writeDocument(id: UUID, content: String) -> FfiResult<Empty, WriteToDocumentError>
    func deleteFile(id: UUID) -> FfiResult<Empty, FileDeleteError>
    func renameFile(id: UUID, name: String) -> FfiResult<Empty, RenameFileError>
    func moveFile(id: UUID, newParent: UUID) -> FfiResult<Empty, MoveFileError>
    func shareFile(id: UUID, username: String, isWrite: Bool) -> FfiResult<Empty, ShareFileError>
    func getPendingShares() -> FfiResult<[File], GetPendingShares>
    func deletePendingShare(id: UUID) ->FfiResult<Empty, DeletePendingShareError>
    func exportFile(id: UUID, destination: String) ->FfiResult<Empty, ExportFileError>
    func importFiles(sources: [String], destination: UUID) ->FfiResult<Empty, ImportFilesError>
    func getFileById(id: UUID) -> FfiResult<File, GetFileByIdError>
    func getFileByPath(path: String) -> FfiResult<File, GetFileByPathError>
    func suggestedDocs() -> FfiResult<[UUID], SuggestedDocsError>
    func getPathById(id: UUID) -> FfiResult<String, GetPathByIdError>
    func debugInfo() -> String
    
    func timeAgo(timeStamp: Int64) -> String
    
    // Billing
    func newAppleSub(originalTransactionId: String, appAccountToken: String) -> FfiResult<Empty, UpgradeAccountAppStoreError>
    func cancelSub() -> FfiResult<Empty, CancelSubscriptionError>
    
    // Search
    func startSearch(
        isPathAndContentSearch: Bool,
        context: UnsafeRawPointer?,
        // updateStatus(context, searchResultType, searchResultJson)
        updateStatus: @escaping @convention(c) (UnsafePointer<Int8>?, Int32, UnsafePointer<Int8>?) -> Void
    ) -> FfiResult<Empty, GeneralSearchError>
    func endSearch(isPathAndContentSearch: Bool) -> FfiResult<Empty, GeneralSearchError>
    func searchQuery(query: String, isPathAndContentSearch: Bool) -> FfiResult<Empty, GeneralSearchError>
    
    func freeText(s: UnsafePointer<Int8>)
}
