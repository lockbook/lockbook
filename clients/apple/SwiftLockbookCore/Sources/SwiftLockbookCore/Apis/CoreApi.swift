import Foundation
import CLockbookCore

public struct CoreApi: LockbookApi {

    var documentsDirectory: String
    
    public init(_ documentsDirectory: String, logs: Bool) {
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
    
    public func syncAll() -> FfiResult<Empty, SyncAllError> {
        fromPrimitiveResult(result: sync_all())
    }
    
    public func calculateWork() -> FfiResult<WorkCalculated, CalculateWorkError> {
        fromPrimitiveResult(result: calculate_work())
    }
    
    public func getRoot() -> FfiResult<File, GetRootError> {
        fromPrimitiveResult(result: get_root())
    }
    
    public func listFiles() -> FfiResult<[File], ListMetadatasError> {
        fromPrimitiveResult(result: list_metadatas())
    }
    
    // TODO this needs to be renamed
    public func getFile(id: UUID) -> FfiResult<String, ReadDocumentError> {
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
    
    // TODO this needs to be renamed and brought in line with core
    public func updateFile(id: UUID, content: String) -> FfiResult<Empty, WriteToDocumentError> {
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
}
