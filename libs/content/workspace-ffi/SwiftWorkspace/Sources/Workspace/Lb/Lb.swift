import Bridge
import Foundation
import SwiftUI

public protocol LbAPI {
    var lbUnsafeRawPtr: UnsafeMutableRawPointer? { get set }
    var events: Events { get }

    func createAccount(username: String, apiUrl: String?, welcomeDoc: Bool) -> Result<Account, LbError>
    func importAccount(key: String, apiUrl: String?) -> Result<Account, LbError>
    func getAccount() -> Result<Account, LbError>
    func deleteAccount() -> Result<Void, LbError>
    func logoutAndExit()
    func exportAccountPrivateKey() -> Result<String, LbError>
    func exportAccountPhrase() -> Result<String, LbError>
    func createFile(name: String, parent: UUID, fileType: FileType) -> Result<File, LbError>
    func createLink(name: String, parent: UUID, target: UUID) -> Result<File, LbError>
    func getFile(id: UUID) -> Result<File, LbError>
    func deleteFile(id: UUID) -> Result<Void, LbError>
    func duplicateFile(id: UUID) -> Result<File, LbError>
    func readDoc(id: UUID) -> Result<[UInt8], LbError>
    func listMetadatas() -> Result<[File], LbError>
    func renameFile(id: UUID, newName: String) -> Result<Void, LbError>
    func moveFile(id: UUID, newParent: UUID) -> Result<Void, LbError>
    func shareFile(id: UUID, username: String, mode: ShareMode) -> Result<Void, LbError>
    func getPendingShares() -> Result<[File], LbError>
    func getPendingShareFiles() -> Result<[File], LbError>
    func deletePendingShare(id: UUID) -> Result<Void, LbError>
    func debugInfo() -> String
    func sync() -> Result<Void, LbError>
    func pinFile(id: UUID) -> Result<Void, LbError>
    func unpinFile(id: UUID) -> Result<Void, LbError>
    func listPinned() -> Result<[UUID], LbError>
    func getUsage() -> Result<UsageMetrics, LbError>
    func importFiles(sources: [String], dest: UUID) -> Result<Void, LbError>
    func exportFile(sourceId: UUID, dest: String, edit: Bool) -> Result<Void, LbError>
    func getFileLinkUrl(id: UUID) -> Result<String, LbError>
    func pathSearcher() -> PathSearching
    func contentSearcher() -> ContentSearching
    func upgradeAccountAppStore(originalTransactionId: String, appAccountToken: String) -> Result<Void, LbError>
    func cancelSubscription() -> Result<Void, LbError>
    func getSubscriptionInfo() -> Result<SubscriptionInfo?, LbError>
}

public class Lb: LbAPI {
    public var lb: OpaquePointer?
    public var lbUnsafeRawPtr: UnsafeMutableRawPointer?
    public var events: Events = .init()

    public init(writablePath: String, logs: Bool) {
        print("Starting core at \(writablePath) and logs=\(logs)")

        _ = start(writablePath: writablePath, logs: logs)

        subscribe(notify: { event in
            DispatchQueue.main.async {
                if event.status_updated {
                    self.events.status = self.getStatus()
                } else if event.metadata_updated || event.pending_shares_changed {
                    self.events.metadataVersion += 1
                } else if event.doc_written {
                    self.events.docWritten = DocWritten(
                        id: event.doc_written_id.toUUID(),
                        seq: (self.events.docWritten?.seq ?? 0) + 1
                    )
                }
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

    public func deleteAccount() -> Result<Void, LbError> {
        let err = lb_delete_account(lb)

        if let err {
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

        if let err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }

    public func duplicateFile(id: UUID) -> Result<File, LbError> {
        let res = lb_duplicate_file(lb, id.toLbUuid())
        defer { lb_free_file_res(res) }

        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success(File(res.file))
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

        return .success([LbFile](UnsafeBufferPointer(start: res.list.list, count: Int(res.list.count))).toFiles())
    }

    public func renameFile(id: UUID, newName: String) -> Result<Void, LbError> {
        let err = lb_rename_file(lb, id.toLbUuid(), newName)

        if let err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }

    public func moveFile(id: UUID, newParent: UUID) -> Result<Void, LbError> {
        let err = lb_move_file(lb, id.toLbUuid(), newParent.toLbUuid())

        if let err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }

    public func shareFile(id: UUID, username: String, mode: ShareMode) -> Result<Void, LbError> {
        let err = lb_share_file(lb, id.toLbUuid(), username, mode.toLbShareMode())

        if let err {
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

        return .success([LbFile](UnsafeBufferPointer(start: res.list.list, count: Int(res.list.count))).toFiles())
    }

    public func getPendingShareFiles() -> Result<[File], LbError> {
        let res = lb_get_pending_share_files(lb)
        defer { lb_free_file_list_res(res) }

        guard res.err == nil else {
            return .failure(LbError(res.err.pointee))
        }

        return .success([LbFile](UnsafeBufferPointer(start: res.list.list, count: Int(res.list.count))).toFiles())
    }

    public func deletePendingShare(id: UUID) -> Result<Void, LbError> {
        let err = lb_delete_pending_share(lb, id.toLbUuid())

        if let err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }

    public func debugInfo() -> String {
        let osInfo = ProcessInfo.processInfo.operatingSystemVersion
        let debugInfo = lb_debug_info(lb, "\(osInfo.majorVersion).\(osInfo.minorVersion).\(osInfo.patchVersion)")
        defer { lb_free_str(debugInfo) }

        return String(cString: debugInfo!)
    }

    class UpdateSyncStatus {
        let closure: ((UInt, UInt, UUID, String) -> Void)?

        init(_ closure: ((UInt, UInt, UUID, String) -> Void)?) {
            self.closure = closure
        }

        func toPointer() -> UnsafeRawPointer {
            UnsafeRawPointer(Unmanaged.passRetained(self).toOpaque())
        }

        static func fromPtr(_ pointer: UnsafeRawPointer) -> UpdateSyncStatus {
            Unmanaged<UpdateSyncStatus>.fromOpaque(pointer).takeUnretainedValue()
        }
    }
            
    public func sync() -> Result<Void, LbError> {
        
        let err = lb_sync(lb)
        
        if let err = err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }

    public func pinFile(id: UUID) -> Result<Void, LbError> {
        let err = lb_pin_file(lb, id.toLbUuid())

        if let err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }

    public func unpinFile(id: UUID) -> Result<Void, LbError> {
        let err = lb_unpin_file(lb, id.toLbUuid())

        if let err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }

    public func listPinned() -> Result<[UUID], LbError> {
        let res = lb_list_pinned(lb)
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

        if let err {
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }

    public func exportFile(sourceId: UUID, dest: String, edit: Bool) -> Result<Void, LbError> {
        let err = lb_export_file(lb, sourceId.toLbUuid(), dest, edit)

        if let err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }
    
    public func getFileLinkUrl(id: UUID) -> Result<String, LbError> {
        let res = lb_get_file_link_url(lb, id.toLbUuid())
        defer { lb_free_get_file_link_url_res(res) }

        return .success(String(cString: res.link_url))
    }

    public func pathSearcher() -> PathSearching {
        LbPathSearcher(lb: lb)
    }

    public func contentSearcher() -> ContentSearching {
        LbContentSearcher(lb: lb)
    }

    public func upgradeAccountAppStore(originalTransactionId: String, appAccountToken: String) -> Result<Void, LbError> {
        let err = lb_upgrade_account_app_store(lb, originalTransactionId, appAccountToken)

        if let err {
            defer { lb_free_err(err) }
            return .failure(LbError(err.pointee))
        }

        return .success(())
    }

    public func cancelSubscription() -> Result<Void, LbError> {
        let err = lb_cancel_subscription(lb)

        if let err {
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
            UnsafeRawPointer(Unmanaged.passRetained(self).toOpaque())
        }

        static func fromPtr(_ pointer: UnsafeRawPointer) -> Notify {
            Unmanaged<Notify>.fromOpaque(pointer).takeUnretainedValue()
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
    public var lbUnsafeRawPtr: UnsafeMutableRawPointer?
    public var events: Events = .init()

    public let account = Account(username: "smail", apiUrl: "https://app.lockbook.net")
    public let accountPK = "BQAAAAAAAAB0ZXN0MQkAAAAAAAAAdGVzdDEuY29tIAAAAAAAAAATIlUEJFM0ejFr3ywfEAKgZGfBAEMPuIUhb1uPiejwKg"
    public let accountPhrase = "turkey, era, velvet, detail, prison, income, dose, royal, fever, truly, unique, couple, party, example, piece, art, leaf, follow, rose, access, vacant, gather, wasp, audit"

    public let rootId: UUID
    public let file0: File
    public let file1: File
    public let file2: File
    public let file3: File
    public let file4: File
    public let file5: File

    public init() {
        rootId = UUID()

        file0 = File(id: rootId, parent: rootId, name: "smail", type: .folder, lastModifiedBy: "smail", lastModified: 1_735_857_212, shares: [])
        file1 = File(id: UUID(), parent: rootId, name: "welcome.md", type: .document, lastModifiedBy: "smail", lastModified: 1_735_857_212, shares: [Share(by: "smail", with: "parth", mode: .write), Share(by: "smail", with: "adam", mode: .write), Share(by: "smail", with: "travis", mode: .write), Share(by: "smail", with: "rando", mode: .read)])
        file2 = File(id: UUID(), parent: rootId, name: "about.md", type: .document, lastModifiedBy: "smail", lastModified: 1_735_857_215, shares: [])
        file3 = File(id: UUID(), parent: rootId, name: "projects.md", type: .document, lastModifiedBy: "smail", lastModified: 1_735_857_220, shares: [])
        file4 = File(id: UUID(), parent: rootId, name: "contact.md", type: .document, lastModifiedBy: "smail", lastModified: 1_735_857_225, shares: [])
        file5 = File(id: UUID(), parent: rootId, name: "notes.md", type: .document, lastModifiedBy: "smail", lastModified: 1_735_857_230, shares: [])
    }

    public func createAccount(username _: String, apiUrl _: String?, welcomeDoc _: Bool) -> Result<Account, LbError> {
        .success(account)
    }

    public func importAccount(key _: String, apiUrl _: String?) -> Result<Account, LbError> {
        .success(account)
    }

    public func getAccount() -> Result<Account, LbError> {
        .success(account)
    }

    public func deleteAccount() -> Result<Void, LbError> {
        .success(())
    }

    public func logoutAndExit() {}
    public func exportAccountPrivateKey() -> Result<String, LbError> { .success(accountPK) }
    public func exportAccountPhrase() -> Result<String, LbError> { .success(accountPhrase) }
    public func createFile(name: String, parent: UUID, fileType: FileType) -> Result<File, LbError> { .success(file1) }
    public func createLink(name: String, parent: UUID, target: UUID) -> Result<File, LbError> { .success(File(id: UUID(), parent: UUID(), name: "about-link.md", type: .link(file2.id), lastModifiedBy: "smail", lastModified: 1735857215, shares: [])) }
    public func getFile(id: UUID) -> Result<File, LbError> { .success(file1) }
    public func deleteFile(id: UUID) -> Result<Void, LbError> { .success(()) }
    public func duplicateFile(id: UUID) -> Result<File, LbError> { .success(file1) }
    public func readDoc(id: UUID) -> Result<[UInt8], LbError> { .success([]) }
    public func listMetadatas() -> Result<[File], LbError> { .success([file0, file1, file2, file3, file4, file5]) }
    public func renameFile(id: UUID, newName: String) -> Result<Void, LbError> { .success(()) }
    public func moveFile(id: UUID, newParent: UUID) -> Result<Void, LbError> { .success(()) }
    public func shareFile(id: UUID, username: String, mode: ShareMode) -> Result<Void, LbError> { .success(()) }
    public func getPendingShares() -> Result<[File], LbError> { .success([file1, file1, file1, file1, file1]) }
    public func getPendingShareFiles() -> Result<[File], LbError> { .success([file1, file1, file1, file1, file1]) }
    public func deletePendingShare(id: UUID) -> Result<Void, LbError> { .success(()) }
    public func debugInfo() -> String { "No debug info. This is a preview." }
    public func sync() -> Result<Void, LbError> { .success(()) }
    public func pinFile(id: UUID) -> Result<Void, LbError> { .success(()) }
    public func unpinFile(id: UUID) -> Result<Void, LbError> { .success(()) }
    public func listPinned() -> Result<[UUID], LbError> { .success([file1.id]) }
    public func getUsage() -> Result<UsageMetrics, LbError> { .success(UsageMetrics(serverUsedExact: 100, serverUsedHuman: "100B", serverCapExact: 1000, serverCapHuman: "1000B")) }
    public func importFiles(sources: [String], dest: UUID) -> Result<Void, LbError> { .success(()) }
    public func exportFile(sourceId: UUID, dest: String, edit: Bool) -> Result<Void, LbError> { .success(()) }
    public func getFileLinkUrl(id: UUID) -> Result<String, LbError> {
        .success("https://app.lockbook.net/open/a6743b18-c7ef-4960-9825-8022e2fa5672")
    }
    public func pathSearcher() -> PathSearching { MockPathSearcher() }
    public func contentSearcher() -> ContentSearching { MockContentSearcher() }
    public func upgradeAccountAppStore(originalTransactionId: String, appAccountToken: String) -> Result<Void, LbError> { .success(()) }
    public func cancelSubscription() -> Result<Void, LbError> { .success(()) }
    public func getSubscriptionInfo() -> Result<SubscriptionInfo?, LbError> { .success(nil) }
}
