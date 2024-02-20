import Foundation
import CLockbookCore

public struct CoreApi: LockbookApi {
    var documentsDirectory: String
    
    public init(_ documentsDirectory: String, logs: Bool) {
        print(FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).map({ url in url.path }))
        
        self.documentsDirectory = documentsDirectory
        print("Located at \(documentsDirectory)")
        print("core init result: \(startCore(logs))")
    }
    
    public func startCore(_ logs: Bool) -> FfiResult<Empty, InitError> {
        fromPrimitiveResult(result: `init`(documentsDirectory, logs))
    }
    
    public func getAccount() -> FfiResult<Account, GetAccountError> {
        fromPrimitiveResult(result: get_account())
    }
    
    public func createAccount(username: String, apiLocation: String, welcomeDoc: Bool) -> FfiResult<Empty, CreateAccountError> {
        fromPrimitiveResult(result: create_account(username, apiLocation, welcomeDoc))
    }
    
    public func clearLocalDb() -> FfiResult<Empty, ClearLocalDbError> {
        fromPrimitiveResult(result: clear_local_db())
    }
    
    public func importAccount(accountString: String) -> FfiResult<Empty, ImportError> {
        fromPrimitiveResult(result: import_account(accountString.trimmingCharacters(in: .whitespacesAndNewlines)))
    }
    
    public func exportAccount() -> FfiResult<String, AccountExportError> {
        fromPrimitiveResult(result: export_account())
    }
    
    public func getUsage() -> FfiResult<UsageMetrics, GetUsageError> {
        fromPrimitiveResult(result: get_usage())
    }
    
    public func getUncompressedUsage() -> FfiResult<UsageItemMetric, GetUsageError> {
        fromPrimitiveResult(result: get_uncompressed_usage())
    }
    
    public func deleteAccount() -> FfiResult<Empty, DeleteAccountError> {
        fromPrimitiveResult(result: delete_account())
    }
    
    public func syncAll(context: UnsafeRawPointer?, updateStatus: @escaping @convention(c) (UnsafePointer<Int8>?, UnsafePointer<Int8>?, Float) -> Void) -> FfiResult<Empty, SyncAllError> {
        fromPrimitiveResult(result: sync_all(context, updateStatus))
    }
    
    public func backgroundSync() -> FfiResult<Empty, SyncAllError> {
        fromPrimitiveResult(result: background_sync())
    }
    
    public func calculateWork() -> FfiResult<SyncStatus, CalculateWorkError> {
        fromPrimitiveResult(result: calculate_work())
    }
    
    public func getRoot() -> FfiResult<File, GetRootError> {
        fromPrimitiveResult(result: get_root())
    }
    
    public func listFiles() -> FfiResult<[File], ListMetadatasError> {
        fromPrimitiveResult(result: list_metadatas())
    }
    
    public func readDocument(id: UUID) -> FfiResult<String, ReadDocumentError> {
        fromPrimitiveResult(result: read_document(id.uuidString))
    }
    
    public func readDrawing(id: UUID) -> FfiResult<Drawing, GetDrawingError> {
        fromPrimitiveResult(result: get_drawing(id.uuidString))
    }
    
    public func writeDrawing(id: UUID, content: Drawing) -> FfiResult<Empty, WriteToDocumentError> {
        switch serialize(obj: content) {
        case .success(let serializedDrawing):
            return fromPrimitiveResult(result: write_document(id.uuidString, serializedDrawing))
        case .failure(let err):
            return .failure(.init(unexpected: err.localizedDescription))
        }
    }

    public func exportDrawing(id: UUID) -> FfiResult<Data, ExportDrawingError> {
        let res: FfiResult<[UInt8], ExportDrawingError> = fromPrimitiveResult(result: export_drawing(id.uuidString))
        return res.map(transform: { Data($0) })
    }
    
    public func createFile(name: String, dirId: UUID, isFolder: Bool) -> FfiResult<File, CreateFileError> {
        let fileType = isFolder ? "Folder" : "Document"
        return fromPrimitiveResult(result: create_file(name, dirId.uuidString, fileType))
    }
    
    public func createLink(name: String, dirId: UUID, target: UUID) -> FfiResult<Empty, CreateFileError> {
        fromPrimitiveResult(result: create_link(name, dirId.uuidString, target.uuidString))
    }
    
    public func writeDocument(id: UUID, content: String) -> FfiResult<Empty, WriteToDocumentError> {
        fromPrimitiveResult(result: write_document(id.uuidString, content))
    }
    
    public func deleteFile(id: UUID) -> FfiResult<Empty, FileDeleteError> {
        fromPrimitiveResult(result: delete_file(id.uuidString))
    }
    
    public func renameFile(id: UUID, name: String) -> FfiResult<Empty, RenameFileError> {
        fromPrimitiveResult(result: rename_file(id.uuidString, name))
    }

    public func moveFile(id: UUID, newParent: UUID) -> FfiResult<Empty, MoveFileError> {
        fromPrimitiveResult(result: move_file(id.uuidString, newParent.uuidString))
    }

    public func getLocalChanges() -> FfiResult<[UUID], GetLocalChangesError> {
        fromPrimitiveResult(result: get_local_changes())
    }
    
    public func getLastSyncedHumanString() -> FfiResult<String, GetLastSyncedError> {
        fromPrimitiveResult(result: get_last_synced_human_string())
    }
    
    public func newAppleSub(originalTransactionId: String, appAccountToken: String) -> FfiResult<Empty, UpgradeAccountAppStoreError> {
        fromPrimitiveResult(result: upgrade_account_app_store(originalTransactionId, appAccountToken))
    }

    public func cancelSub() -> FfiResult<Empty, CancelSubscriptionError> {
        fromPrimitiveResult(result: cancel_subscription())
    }
    
    public func shareFile(id: UUID, username: String, isWrite: Bool) -> FfiResult<Empty, ShareFileError> {
        let shareMode = isWrite ? "Write" : "Read"
        return fromPrimitiveResult(result: share_file(id.uuidString, username, shareMode))
    }
    
    public func getPendingShares() -> FfiResult<[File], GetPendingShares> {
        fromPrimitiveResult(result: get_pending_shares())
    }
    
    public func deletePendingShare(id: UUID) ->FfiResult<Empty, DeletePendingShareError> {
        fromPrimitiveResult(result: delete_pending_share(id.uuidString))
    }

    public func exportFile(id: UUID, destination: String) ->FfiResult<Empty, ExportFileError> {
        fromPrimitiveResult(result:  export_file(id.uuidString, destination))
    }
    
    public func exportDrawingToDisk(id: UUID, destination: String) ->FfiResult<Empty, ExportDrawingToDiskError> {
        fromPrimitiveResult(result:  export_drawing_to_disk(id.uuidString, destination))
    }

    public func importFiles(sources: [String], destination: UUID) ->FfiResult<Empty, ImportFilesError> {
        let encodedSources = String(data: try! JSONSerialization.data(withJSONObject: sources), encoding: String.Encoding.utf8)

        return fromPrimitiveResult(result: import_files(encodedSources, destination.uuidString))
    }
    
    public func getFileById(id: UUID) -> FfiResult<File, GetFileByIdError> {
        fromPrimitiveResult(result: get_file_by_id(id.uuidString))
    }
    
    public func getFileByPath(path: String) -> FfiResult<File, GetFileByPathError> {
        fromPrimitiveResult(result: get_by_path(path))
    }
    
    public func startSearch(isPathAndContentSearch: Bool, context: UnsafeRawPointer?, updateStatus: @escaping @convention(c) (UnsafePointer<Int8>?, Int32, UnsafePointer<Int8>?) -> Void) -> FfiResult<Empty, GeneralSearchError> {
        fromPrimitiveResult(result: start_search(isPathAndContentSearch, context, updateStatus))
    }
    
    public func endSearch(isPathAndContentSearch: Bool) -> FfiResult<Empty, GeneralSearchError> {
        fromPrimitiveResult(result: end_search(isPathAndContentSearch))
    }
    
    public func searchQuery(query: String, isPathAndContentSearch: Bool) -> FfiResult<Empty, GeneralSearchError> {
        fromPrimitiveResult(result: search(query, isPathAndContentSearch))
    }
    
    public func suggestedDocs() -> FfiResult<[UUID], SuggestedDocsError> {
        fromPrimitiveResult(result: suggested_docs())
    }
    
    public func getPathById(id: UUID) -> FfiResult<String, GetPathByIdError> {
        fromPrimitiveResult(result: get_path_by_id(id.uuidString))
    }
    
    public func timeAgo(timeStamp: Int64) -> String {
        let msgPointer = time_ago(timeStamp)
        
        let msg = String(cString: msgPointer!)
        release_pointer(UnsafeMutablePointer(mutating: msgPointer))
        
        return msg
    }
    
    public func freeText(s: UnsafePointer<Int8>) {
        release_pointer(UnsafeMutablePointer(mutating: s))
    }
}
