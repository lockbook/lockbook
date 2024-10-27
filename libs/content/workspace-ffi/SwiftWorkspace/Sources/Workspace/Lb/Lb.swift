import Bridge
import Foundation

public class Lb {
    public var lb: OpaquePointer? = nil
            
    public func start(writablePath: String, logs: Bool) -> Result<Void, LbError> {
        let res = lb_init(writablePath, logs)
        defer { lb_free_init(res) }
                
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        lb = res.lb
        return .success(())
    }
        
    public func createAccount(username: String, apiUrl: String, welcomeDoc: Bool) -> Result<Account, LbError> {
        let res = lb_create_account(lb, username, apiUrl, welcomeDoc)
        defer { lb_free_account(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Account(res))
    }
    
    public func importAccount(key: String, apiUrl: String) -> Result<Account, LbError> {
        let res = lb_import_account(lb, key, apiUrl)
        defer { lb_free_account(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Account(res))
    }
    
    public func getAccount() -> Result<Account, LbError> {
        let res = lb_get_account(lb)
        defer { lb_free_account(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Account(res))
    }
    
    public func deleteAccount() -> Result<(), LbError> {
        let err = lb_delete_account(lb)
        defer { lb_free_err(err) }
        
        if let err = err {
            return .failure(LbError(err.pointee))
        }
        
        return .success(())
    }
    
    public func logoutAndExit() {
        lb_logout_and_exit(lb)
    }
    
    public func exportAccountPrivateKey() -> Result<String, LbError> {
        let res = lb_export_account_private_key(lb)
        defer { lb_free_export_account(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(String(cString: res.account_string))
    }
    
    public func exportAccountPhrase() -> Result<String, LbError> {
        let res = lb_export_account_phrase(lb)
        defer { lb_free_export_account(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(String(cString: res.account_string))
    }
    
    public func exportAccountQR() -> Result<[UInt8], LbError> {
        let res = lb_export_account_qr(lb)
        defer { lb_free_export_account_qr(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Array(UnsafeBufferPointer(start: res.qr, count: Int(res.qr_len))))
    }

    public func createFile(name: String, parent: UUID, fileType: FileType) -> Result<File, LbError> {
        let res = lb_create_file(lb, name, parent.toLbUuid(), fileType.toLbFileType())
        defer { lb_free_file_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(File(res.file))
    }
    
    public func writeDcoument(id: UUID, content: inout [UInt8]) -> Result<(), LbError> {
        let len = UInt(content.count)
        let ptr = content.withUnsafeMutableBytes {
            $0.baseAddress?.assumingMemoryBound(to: UInt8.self)
        }
        
        let err = lb_write_document(lb, id.toLbUuid(), ptr, len)
        defer { lb_free_err(err) }
        
        if let err = err {
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func getRoot() -> Result<File, LbError> {
        let res = lb_get_root(lb)
        defer { lb_free_file_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(File(res.file))
    }
    
    public func getChildren(id: UUID) -> Result<[File], LbError> {
        let res = lb_get_children(lb, id.toLbUuid())
        defer { lb_free_file_list_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Array<LbFile>(UnsafeBufferPointer(start: res.list.list, count: Int(res.list.count))).toFiles())
    }
    
    public func getAndGetChildren(id: UUID) -> Result<[File], LbError> {
        let res = lb_get_and_get_children_recursively(lb, id.toLbUuid())
        defer { lb_free_file_list_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Array<LbFile>(UnsafeBufferPointer(start: res.list.list, count: Int(res.list.count))).toFiles())
    }
    
    public func getFile(id: UUID) -> Result<File, LbError> {
        let res = lb_get_file(lb, id.toLbUuid())
        defer { lb_free_file_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(File(res.file))
    }
    
    public func deleteFile(id: UUID) -> Result<Void, LbError> {
        let err = lb_delete_file(lb, id.toLbUuid())
        defer { lb_free_err(err) }
        
        if let err = err {
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func readDoc(id: UUID) -> Result<[UInt8], LbError> {
        let res = lb_read_doc(lb, id.toLbUuid())
        defer { lb_free_doc_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Array(UnsafeBufferPointer(start: res.doc, count: Int(res.len))))
    }
    
    public func listMetadatas() -> Result<[File], LbError> {
        let res = lb_list_metadatas(lb)
        defer { lb_free_file_list_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Array<LbFile>(UnsafeBufferPointer(start: res.list.list, count: Int(res.list.count))).toFiles())
    }
    
    public func renameFile(id: UUID, newName: String) -> Result<Void, LbError> {
        let err = lb_rename_file(lb, id.toLbUuid(), newName)
        defer { lb_free_err(err) }
        
        if let err = err {
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func moveFile(id: UUID, newParent: UUID) -> Result<Void, LbError> {
        let err = lb_move_file(lb, id.toLbUuid(), newParent.toLbUuid())
        defer { lb_free_err(err) }
        
        if let err = err {
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func shareFile(id: UUID, username: String, mode: ShareMode) -> Result<Void, LbError> {
        let err = lb_share_file(lb, id.toLbUuid(), username, mode)
        defer { lb_free_err(err) }
        
        if let err = err {
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func getPendingShares() -> Result<[File], LbError> {
        let res = lb_get_pending_shares(lb)
        defer { lb_free_file_list_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Array<LbFile>(UnsafeBufferPointer(start: res.list.list, count: Int(res.list.count))).toFiles())
    }

    public func createLinkAtPath(pathAndName: String, targetId: UUID) -> Result<File, LbError> {
        let res = lb_create_link_at_path(lb, pathAndName, targetId.toLbUuid())
        defer { lb_free_file_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(File(res.file))
    }
    
    public func createAtPath(pathAndName: String) -> Result<File, LbError> {
        let res = lb_create_at_path(lb, pathAndName)
        defer { lb_free_file_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(File(res.file))
    }
    
    public func getByPath(path: String) -> Result<File, LbError> {
        let res = lb_get_by_path(lb, path)
        defer { lb_free_file_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(File(res.file))
    }
    
    public func getPathById(id: UUID) -> Result<String, LbError> {
        let res = lb_get_path_by_id(lb, id.toLbUuid())
        defer { lb_free_path_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(String(cString: res.path))
    }
    
    public func getLocalChanges() -> Result<[UUID], LbError> {
        let res = lb_get_local_changes(lb)
        defer { lb_free_id_list_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Array(UnsafeBufferPointer(start: res.ids, count: Int(res.len))).toUUIDs())
    }
    
    public func debugInfo(osInfo: String) -> String {
        let debugInfo = lb_debug_info(lb, osInfo)
        defer { lb_free_str(debugInfo) }
        
        return String(cString: debugInfo!)
    }
    
    public func calculateWork() -> Result<SyncStatus, LbError> {
        let res = lb_calculate_work(lb)
        defer { lb_free_sync_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(SyncStatus(res))
    }
    
    public func get_last_synced() -> Result<Int64, LbError> {
        let res = lb_get_last_synced(lb)
        defer { lb_free_last_synced_i64(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(res.last)
    }
    
    public func get_last_synced_human_string() -> Result<String, LbError> {
        let res = lb_get_last_synced_human_string(lb)
        defer { lb_free_last_synced_human(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(String(cString: res.last))
    }
    
    public func suggestedDocs() -> Result<[UUID], LbError> {
        let res = lb_suggested_docs(lb)
        defer { lb_free_id_list_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Array(UnsafeBufferPointer(start: res.ids, count: Int(res.len))).toUUIDs())
    }
    
    public func getUsage() -> Result<UsageMetrics, LbError> {
        let res = lb_get_usage(lb)
        defer { lb_free_usage_metrics(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(UsageMetrics(res.usages))
    }
    
    public func getUncompressedUsage() -> Result<UncompressedUsageMetric, LbError> {
        let res = lb_get_uncompressed_usage(lb)
        defer { lb_free_uncompressed_usage(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(UncompressedUsageMetric(res))
    }
    
    public func importFiles(sources: [String], dest: UUID) -> Result<Void, LbError> {
        let sourcesLen = UInt(sources.count)
        var sources = sources.map { strdup($0) }
        let sourcesPtr = sources.withUnsafeBytes {
            $0.baseAddress?.assumingMemoryBound(to: Optional<UnsafePointer<CChar>>.self)
        }
        
        let err = lb_import_files(lb, sourcesPtr, sourcesLen, dest.toLbUuid())
        defer {
            lb_free_err(err)
            sources.forEach({ free($0) })
        }
        
        if let err = err {
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func exportFile(sourceId: UUID, dest: String, edit: Bool) -> Result<Void, LbError> {
        let err = lb_export_file(lb, sourceId.toLbUuid(), dest, edit)
        defer { lb_free_err(err) }
        
        if let err = err {
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    func search(input: String, searchPaths: Bool, searchDocs: Bool) -> Result<([PathSearchResult], [DocumentSearchResult]), LbError> {
        let res = lb_search(lb, input, searchPaths, searchDocs)
        defer { lb_free_search_results(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success((Array(UnsafeBufferPointer(start: res.path_results, count: Int(res.path_results_len))).toPathSearchResults(), Array(UnsafeBufferPointer(start: res.document_results, count: Int(res.document_results_len))).toDocumentSearchResults()))
    }
    
    func upgradeAccountStripe(isOldCard: Bool, number: String, expYear: Int32, expMonth: Int32, cvc: String) -> Result<Void, LbError> {
        let err = lb_upgrade_account_stripe(lb, isOldCard, number, expYear, expMonth, cvc)
        defer { lb_free_err(err) }
        
        if let err = err {
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    func upgradeAccountStripe(originalTransactionId: String, appAccountToken: String) -> Result<Void, LbError> {
        let err = lb_upgrade_account_app_store(lb, originalTransactionId, appAccountToken)
        defer { lb_free_err(err) }
        
        if let err = err {
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    func upgradeAccountStripe() -> Result<Void, LbError> {
        let err = lb_cancel_subscription(lb)
        defer { lb_free_err(err) }
        
        if let err = err {
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    func getSubscriptionInfo() -> Result<SubscriptionInfo?, LbError> {
        let res = lb_get_subscription_info(lb)
        defer { lb_free_sub_info(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        if res.info != nil {
            return .success(SubscriptionInfo(res.info.pointee))
        } else {
            return .success(nil)
        }
    }
}
