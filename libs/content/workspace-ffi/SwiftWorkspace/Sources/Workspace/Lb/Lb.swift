import Bridge
import Foundation
import SwiftUI
import Combine

public protocol LbAPI {
    var lb: OpaquePointer? { get set }
    var lbUnsafeRawPtr: UnsafeMutableRawPointer? { get set }
    var events: Events { get }
    
    func start(writablePath: String, logs: Bool) -> Result<Void, LbError>
    func createAccount(username: String, apiUrl: String?, welcomeDoc: Bool) -> Result<Account, LbError>
    func importAccount(key: String, apiUrl: String?) -> Result<Account, LbError>
    func getAccount() -> Result<Account, LbError>
    func deleteAccount() -> Result<Void, LbError>
    func logoutAndExit()
    func exportAccountPrivateKey() -> Result<String, LbError>
    func exportAccountPhrase() -> Result<String, LbError>
    func exportAccountQR() -> Result<[UInt8], LbError>
    func createFile(name: String, parent: UUID, fileType: FileType) -> Result<File, LbError>
    func createLink(name: String, parent: UUID, target: UUID) -> Result<File, LbError>
    func writeDocument(id: UUID, content: inout [UInt8]) -> Result<Void, LbError>
    func getRoot() -> Result<File, LbError>
    func getChildren(id: UUID) -> Result<[File], LbError>
    func getAndGetChildren(id: UUID) -> Result<[File], LbError>
    func getFile(id: UUID) -> Result<File, LbError>
    func deleteFile(id: UUID) -> Result<Void, LbError>
    func readDoc(id: UUID) -> Result<[UInt8], LbError>
    func listMetadatas() -> Result<[File], LbError>
    func renameFile(id: UUID, newName: String) -> Result<Void, LbError>
    func moveFile(id: UUID, newParent: UUID) -> Result<Void, LbError>
    func shareFile(id: UUID, username: String, mode: ShareMode) -> Result<Void, LbError>
    func getPendingShares() -> Result<[File], LbError>
    func deletePendingShare(id: UUID) -> Result<Void, LbError>
    func createLinkAtPath(pathAndName: String, targetId: UUID) -> Result<File, LbError>
    func createAtPath(pathAndName: String) -> Result<File, LbError>
    func getByPath(path: String) -> Result<File, LbError>
    func getPathById(id: UUID) -> Result<String, LbError>
    func listFolderPaths() -> Result<[String], LbError>
    func getLocalChanges() -> Result<[UUID], LbError>
    func debugInfo() -> String
    func calculateWork() -> Result<SyncStatus, LbError>
    func sync(updateStatus: ((UInt, UInt, UUID, String) -> Void)?) -> Result<SyncStatus, LbError>
    func getLastSynced() -> Result<Int64, LbError>
    func getLastSyncedHumanString() -> Result<String, LbError>
    func getTimestampHumanString(timestamp: Int64) -> String
    func suggestedDocs() -> Result<[UUID], LbError>
    func clearSuggestedId(id: UUID) -> Result<Void, LbError>
    func clearSuggestedDocs() -> Result<Void, LbError>
    func getUsage() -> Result<UsageMetrics, LbError>
    func getUncompressedUsage() -> Result<UncompressedUsageMetric, LbError>
    func importFiles(sources: [String], dest: UUID) -> Result<Void, LbError>
    func exportFile(sourceId: UUID, dest: String, edit: Bool) -> Result<Void, LbError>
    func search(input: String, searchPaths: Bool, searchDocs: Bool) -> Result<[SearchResult], LbError>
    func upgradeAccountStripe(isOldCard: Bool, number: String, expYear: Int32, expMonth: Int32, cvc: String) -> Result<Void, LbError>
    func upgradeAccountAppStore(originalTransactionId: String, appAccountToken: String) -> Result<Void, LbError>
    func cancelSubscription() -> Result<Void, LbError>
    func getSubscriptionInfo() -> Result<SubscriptionInfo?, LbError>
    func subscribe(notify: ((LbEvent) -> Void)?)
}

public class Lb: LbAPI {
    public var lb: OpaquePointer? = nil
    public var lbUnsafeRawPtr: UnsafeMutableRawPointer? = nil
    public var events: Events = Events()

    public init(writablePath: String, logs: Bool) {
        print("Starting core at \(writablePath) and logs=\(logs)")
        
        let res = start(writablePath: writablePath, logs: logs)
        
        subscribe(notify: { event in
            if event.status_updated {
                self.events.status = self.getStatus()
            } else if event.metadata_updated {
                self.events.metadataUpdated = true
            } else if event.pending_shares_changed {
                self.events.pendingShares = (try? self.getPendingShares().get())?.map(\.id) ?? []
            }
        })
    }
            
    public func start(writablePath: String, logs: Bool) -> Result<Void, LbError> {
        let res = lb_init(writablePath, logs)
        defer {
            lb_free_err(res.err)
        }
                
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        lb = res.lb
        lbUnsafeRawPtr = UnsafeMutableRawPointer(lb!)
        
        return .success(())
    }
        
    public func createAccount(username: String, apiUrl: String?, welcomeDoc: Bool) -> Result<Account, LbError> {
        let res = lb_create_account(lb, username, apiUrl, welcomeDoc)
        defer { lb_free_account(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Account(res))
    }
    
    public func importAccount(key: String, apiUrl: String?) -> Result<Account, LbError> {
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
        
        if let err = err {
            defer { lb_free_err(err) }
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
    
    public func createLink(name: String, parent: UUID, target: UUID) -> Result<File, LbError> {
        let res = lb_create_file(lb, name, parent.toLbUuid(), LbFileType(tag: LbFileTypeTag(2), link_target: target.toLbUuid()))
        defer { lb_free_file_res(res) }

        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(File(res.file))
    }
    
    public func writeDocument(id: UUID, content: inout [UInt8]) -> Result<(), LbError> {
        let len = UInt(content.count)
        let ptr = content.withUnsafeMutableBytes {
            $0.baseAddress?.assumingMemoryBound(to: UInt8.self)
        }
        
        let err = lb_write_document(lb, id.toLbUuid(), ptr, len)
        
        if let err = err {
            defer { lb_free_err(err) }
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
        
        if let err = err {
            defer { lb_free_err(err) }
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
        
        if let err = err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func moveFile(id: UUID, newParent: UUID) -> Result<Void, LbError> {
        let err = lb_move_file(lb, id.toLbUuid(), newParent.toLbUuid())
        
        if let err = err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func shareFile(id: UUID, username: String, mode: ShareMode) -> Result<Void, LbError> {
        let err = lb_share_file(lb, id.toLbUuid(), username, mode.toLbShareMode())
        
        if let err = err {
            defer { lb_free_err(err) }
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
    
    public func deletePendingShare(id: UUID) -> Result<Void, LbError> {
        let err = lb_delete_pending_share(lb, id.toLbUuid())
        
        if let err = err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
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
    
    public func listFolderPaths() -> Result<[String], LbError> {
        let res = lb_list_folder_paths(lb)
        defer { lb_free_paths_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success((0..<res.len).map({ String(cString: res.paths[Int($0)]!) }))
    }
        
    public func getLocalChanges() -> Result<[UUID], LbError> {
        let res = lb_get_local_changes(lb)
        defer { lb_free_id_list_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Array(UnsafeBufferPointer(start: res.ids, count: Int(res.len))).toUUIDs())
    }
    
    public func debugInfo() -> String {
        let osInfo = ProcessInfo.processInfo.operatingSystemVersion
        let debugInfo = lb_debug_info(lb, "\(osInfo.majorVersion).\(osInfo.minorVersion).\(osInfo.patchVersion)")
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
    
    class UpdateSyncStatus {
        let closure: ((UInt, UInt, UUID, String) -> Void)?

        init(_ closure: ((UInt, UInt, UUID, String) -> Void)?) {
            self.closure = closure
        }
        
        func toPointer() -> UnsafeRawPointer {
            return UnsafeRawPointer(Unmanaged.passRetained(self).toOpaque())
        }
        
        static func fromPtr(_ pointer: UnsafeRawPointer) -> UpdateSyncStatus {
            return Unmanaged<UpdateSyncStatus>.fromOpaque(pointer).takeUnretainedValue()
        }
    }
            
    public func sync(updateStatus: ((UInt, UInt, UUID, String) -> Void)?) -> Result<SyncStatus, LbError> {
        var lbUpdateStatusFunc: (@convention(c) (UnsafeRawPointer?, UInt, UInt, LbUuid, UnsafePointer<CChar>?) -> Void)? = nil
        var updateStatusObj: UpdateSyncStatus? = nil
        
        if updateStatus != nil {
            lbUpdateStatusFunc = { (obj: UnsafeRawPointer?, total: UInt, progress: UInt, id: LbUuid, msg: UnsafePointer<CChar>?) in
                UpdateSyncStatus.fromPtr(obj!).closure!(total, progress, id.toUUID(), String(cString: msg!))
            }
            
            updateStatusObj = UpdateSyncStatus(updateStatus)
        }
        
        let res = lb_sync(lb, updateStatusObj?.toPointer(), &lbUpdateStatusFunc)
        defer { lb_free_sync_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(SyncStatus(res))
    }
    
    public func getLastSynced() -> Result<Int64, LbError> {
        let res = lb_get_last_synced(lb)
        defer { lb_free_last_synced_i64(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(res.last)
    }
    
    public func getLastSyncedHumanString() -> Result<String, LbError> {
        let res = lb_get_last_synced_human_string(lb)
        defer { lb_free_last_synced_human(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(String(cString: res.last))
    }
    
    public func getTimestampHumanString(timestamp: Int64) -> String {
        let msg = lb_get_timestamp_human_string(lb, timestamp)
        defer { lb_free_str(msg) }
        
        if let msg = msg {
            return String(cString: msg)
        } else {
            return ""
        }
    }
    
    public func suggestedDocs() -> Result<[UUID], LbError> {
        let res = lb_suggested_docs(lb)
        defer { lb_free_id_list_res(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(Array(UnsafeBufferPointer(start: res.ids, count: Int(res.len))).toUUIDs())
    }
    
    public func clearSuggestedId(id: UUID) -> Result<Void, LbError> {
        let err = lb_clear_suggested_id(lb, id.toLbUuid())
        
        if let err = err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func clearSuggestedDocs() -> Result<Void, LbError> {
        let err = lb_clear_suggested(lb)
        
        if let err = err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
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
        let sources = sources.map { strdup($0) }
        let sourcesPtr = sources.withUnsafeBufferPointer {
            $0.baseAddress?.withMemoryRebound(to: UnsafePointer<CChar>?.self, capacity: sources.count) { $0 }
        }

        let err = lb_import_files(lb, sourcesPtr, sourcesLen, dest.toLbUuid())
        defer {
            lb_free_err(err)
            sources.forEach { free($0) }
        }
        
        if let err = err {
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func exportFile(sourceId: UUID, dest: String, edit: Bool) -> Result<Void, LbError> {
        let err = lb_export_file(lb, sourceId.toLbUuid(), dest, edit)
        
        if let err = err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func search(input: String, searchPaths: Bool, searchDocs: Bool) -> Result<[SearchResult], LbError> {
        let res = lb_search(lb, input, searchPaths, searchDocs)
        defer { lb_free_search_results(res) }
        
        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }
        
        return .success(Array(UnsafeBufferPointer(start: res.results, count: Int(res.results_len))).toSearchResults())
    }
    
    public func upgradeAccountStripe(isOldCard: Bool, number: String, expYear: Int32, expMonth: Int32, cvc: String) -> Result<Void, LbError> {
        let err = lb_upgrade_account_stripe(lb, isOldCard, number, expYear, expMonth, cvc)
        
        if let err = err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func upgradeAccountAppStore(originalTransactionId: String, appAccountToken: String) -> Result<Void, LbError> {
        let err = lb_upgrade_account_app_store(lb, originalTransactionId, appAccountToken)
        
        if let err = err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func cancelSubscription() -> Result<Void, LbError> {
        let err = lb_cancel_subscription(lb)
        
        if let err = err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func getSubscriptionInfo() -> Result<SubscriptionInfo?, LbError> {
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
    
    public func getStatus() -> Status {
        let res = lb_get_status(lb)
        defer { lb_free_status(res) }
        
        return Status(res)
    }
    
    class Notify {
        let closure: ((LbEvent) -> Void)?

        init(_ closure: ((LbEvent) -> Void)?) {
            self.closure = closure
        }
        
        func toPointer() -> UnsafeRawPointer {
            return UnsafeRawPointer(Unmanaged.passRetained(self).toOpaque())
        }
        
        static func fromPtr(_ pointer: UnsafeRawPointer) -> Notify {
            return Unmanaged<Notify>.fromOpaque(pointer).takeUnretainedValue()
        }
    }
        
    public func subscribe(notify: ((LbEvent) -> Void)?) {
        let notifyObj = Notify(notify).toPointer()
        let lbNotifyFunc: LbNotify = { (obj: UnsafeRawPointer?, event: LbEvent) in
            Notify.fromPtr(obj!).closure!(event)
        }
        
        lb_subscribe(lb, notifyObj, lbNotifyFunc)
    }
}

public class MockLb: LbAPI {
    
    @Published public var status: Status = Status()
    public var statusPublisher: Published<Status>.Publisher { $status }
    
    public var lb: OpaquePointer? = nil
    public var lbUnsafeRawPtr: UnsafeMutableRawPointer? = nil
    public var events: Events = Events()
    
    public let account = Account(username: "smail", apiUrl: "https://api.prod.lockbook.net")
    public let accountPK = "BQAAAAAAAAB0ZXN0MQkAAAAAAAAAdGVzdDEuY29tIAAAAAAAAAATIlUEJFM0ejFr3ywfEAKgZGfBAEMPuIUhb1uPiejwKg"
    public let accountPhrase = "turkey, era, velvet, detail, prison, income, dose, royal, fever, truly, unique, couple, party, example, piece, art, leaf, follow, rose, access, vacant, gather, wasp, audit"

    public let syncStatus = SyncStatus(latestServerTS: 1735857215, work: [])
    
    public let rootId: UUID
    public let file0: File
    public let file1: File
    public let file2: File
    public let file3: File
    public let file4: File
    public let file5: File

    public init() {
        self.rootId = UUID()
        
        self.file0 = File(id: rootId, parent: rootId, name: "smail", type: .folder, lastModifiedBy: "smail", lastModified: 1735857212, shares: [])
        self.file1 = File(id: UUID(), parent: rootId, name: "welcome.md", type: .document, lastModifiedBy: "smail", lastModified: 1735857212, shares: [Share(by: "smail", with: "parth", mode: .write), Share(by: "smail", with: "adam", mode: .write), Share(by: "smail", with: "travis", mode: .write), Share(by: "smail", with: "rando", mode: .read)])
        self.file2 = File(id: UUID(), parent: rootId, name: "about.md", type: .document, lastModifiedBy: "smail", lastModified: 1735857215, shares: [])
        self.file3 = File(id: UUID(), parent: rootId, name: "projects.md", type: .document, lastModifiedBy: "smail", lastModified: 1735857220, shares: [])
        self.file4 = File(id: UUID(), parent: rootId, name: "contact.md", type: .document, lastModifiedBy: "smail", lastModified: 1735857225, shares: [])
        self.file5 = File(id: UUID(), parent: rootId, name: "notes.md", type: .document, lastModifiedBy: "smail", lastModified: 1735857230, shares: [])
    }

    public func start(writablePath: String, logs: Bool) -> Result<Void, LbError> { .success(()) }
    public func createAccount(username: String, apiUrl: String?, welcomeDoc: Bool) -> Result<Account, LbError> { .success(account) }
    public func importAccount(key: String, apiUrl: String?) -> Result<Account, LbError> { .success(account) }
    public func getAccount() -> Result<Account, LbError> { .success(account) }
    public func deleteAccount() -> Result<Void, LbError> { .success(()) }
    public func logoutAndExit() {}
    public func exportAccountPrivateKey() -> Result<String, LbError> { .success(accountPK) }
    public func exportAccountPhrase() -> Result<String, LbError> { .success(accountPhrase) }
    public func exportAccountQR() -> Result<[UInt8], LbError> { .success([]) }
    public func createFile(name: String, parent: UUID, fileType: FileType) -> Result<File, LbError> { .success(file1) }
    public func createLink(name: String, parent: UUID, target: UUID) -> Result<File, LbError> { .success(File(id: UUID(), parent: UUID(), name: "about-link.md", type: .link(file2.id), lastModifiedBy: "smail", lastModified: 1735857215, shares: [])) }
    public func writeDocument(id: UUID, content: inout [UInt8]) -> Result<Void, LbError> { .success(()) }
    public func getRoot() -> Result<File, LbError> { .success(file0) }
    public func getChildren(id: UUID) -> Result<[File], LbError> { .success([file1, file2, file3, file4]) }
    public func getAndGetChildren(id: UUID) -> Result<[File], LbError> { .success([file1, file2, file3, file4]) }
    public func getFile(id: UUID) -> Result<File, LbError> { .success(file1) }
    public func deleteFile(id: UUID) -> Result<Void, LbError> { .success(()) }
    public func readDoc(id: UUID) -> Result<[UInt8], LbError> { .success([]) }
    public func listMetadatas() -> Result<[File], LbError> { .success([file0, file1, file2, file3, file4, file5]) }
    public func renameFile(id: UUID, newName: String) -> Result<Void, LbError> { .success(()) }
    public func moveFile(id: UUID, newParent: UUID) -> Result<Void, LbError> { .success(()) }
    public func shareFile(id: UUID, username: String, mode: ShareMode) -> Result<Void, LbError> { .success(()) }
    public func getPendingShares() -> Result<[File], LbError> { .success([file1, file1, file1, file1, file1]) }
    public func deletePendingShare(id: UUID) -> Result<Void, LbError> { .success(()) }
    public func createLinkAtPath(pathAndName: String, targetId: UUID) -> Result<File, LbError> { .success(file5) }
    public func createAtPath(pathAndName: String) -> Result<File, LbError> { .success(file1) }
    public func getByPath(path: String) -> Result<File, LbError> { .success(file1) }
    public func getPathById(id: UUID) -> Result<String, LbError> { .success("/welcome.md") }
    public func listFolderPaths() -> Result<[String], LbError> { .success(["/"]) }
    public func getLocalChanges() -> Result<[UUID], LbError> { .success([UUID(), UUID()]) }
    public func debugInfo() -> String { "No debug info. This is a preview." }
    public func calculateWork() -> Result<SyncStatus, LbError> { .success(syncStatus) }
    public func sync(updateStatus: ((UInt, UInt, UUID, String) -> Void)?) -> Result<SyncStatus, LbError> { .success(syncStatus) }
    public func getLastSynced() -> Result<Int64, LbError> { .success(1735857215) }
    public func getLastSyncedHumanString() -> Result<String, LbError> { .success("You synced a second ago.") }
    public func getTimestampHumanString(timestamp: Int64) -> String { "1 second ago." }
    public func suggestedDocs() -> Result<[UUID], LbError> { .success([file1.id, file2.id, file3.id]) }
    public func clearSuggestedId(id: UUID) -> Result<Void, LbError> { .success(()) }
    public func clearSuggestedDocs() -> Result<Void, LbError> { .success(()) }
    public func getUsage() -> Result<UsageMetrics, LbError> { .success(UsageMetrics(serverUsedExact: 100, serverUsedHuman: "100B", serverCapExact: 1000, serverCapHuman: "1000B")) }
    public func getUncompressedUsage() -> Result<UncompressedUsageMetric, LbError> { .success(UncompressedUsageMetric(exact: 100, humanMsg: "100B")) }
    public func importFiles(sources: [String], dest: UUID) -> Result<Void, LbError> { .success(()) }
    public func exportFile(sourceId: UUID, dest: String, edit: Bool) -> Result<Void, LbError> { .success(()) }
    public func search(input: String, searchPaths: Bool, searchDocs: Bool) -> Result<[SearchResult], LbError> { .success([]) }
    public func upgradeAccountStripe(isOldCard: Bool, number: String, expYear: Int32, expMonth: Int32, cvc: String) -> Result<Void, LbError> { .success(()) }
    public func upgradeAccountAppStore(originalTransactionId: String, appAccountToken: String) -> Result<Void, LbError> { .success(()) }
    public func cancelSubscription() -> Result<Void, LbError> { .success(()) }
    public func getSubscriptionInfo() -> Result<SubscriptionInfo?, LbError> { .success(nil) }
    public func subscribe(notify: ((LbEvent) -> Void)?) { }
}
