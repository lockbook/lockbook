import Foundation
import CLockbookCore

public typealias CoreResult<T> = Result<T, ApplicationError>

public protocol LockbookApi {
    // Account
    func getAccount() -> CoreResult<Account>
    func createAccount(username: String) -> CoreResult<Account>
    func importAccount(accountString: String) -> CoreResult<Account>
    func exportAccount() -> CoreResult<String>
    func getUsage() -> CoreResult<[FileUsage]>
    
    // Work
    func synchronize() -> CoreResult<Empty>
    func calculateWork() -> CoreResult<WorkMetadata>
    func executeWork(work: WorkUnit) -> CoreResult<Empty>
    func setLastSynced(lastSync: UInt64) -> CoreResult<Empty>
    
    // Directory
    func getRoot() -> CoreResult<FileMetadata>
    func listFiles() -> CoreResult<[FileMetadata]>
    
    // Document
    func getFile(id: UUID) -> CoreResult<DecryptedValue>
    func createFile(name: String, dirId: UUID, isFolder: Bool) -> CoreResult<FileMetadata>
    func updateFile(id: UUID, content: String) -> CoreResult<Empty>
    func markFileForDeletion(id: UUID) -> CoreResult<Bool>
    
    // Diagnostic
    func getApiLocation() -> String
}

public struct CoreApi: LockbookApi {
    var documentsDirectory: String
    
    public init(documentsDirectory: String) {
        self.documentsDirectory = documentsDirectory
    }
    
    /// If this isn't called, the rust logger will not start!
    public func initializeLogger() -> Void {
        init_logger_safely(documentsDirectory)
    }
    
    public func getAccount() -> CoreResult<Account> {
        fromPrimitiveResult(result: get_account(documentsDirectory))
    }
    
    public func createAccount(username: String) -> CoreResult<Account> {
        let result: Result<Empty, ApplicationError> = fromPrimitiveResult(result: create_account(documentsDirectory, username))
        
        return result.flatMap { print($0 as Any); return getAccount() }
    }
    
    public func importAccount(accountString: String) -> CoreResult<Account> {
        let result: Result<Empty, ApplicationError> = fromPrimitiveResult(result: import_account(documentsDirectory, accountString.trimmingCharacters(in: .whitespacesAndNewlines)))
        
        return result.flatMap { print($0 as Any); return getAccount() }
    }
    
    public func exportAccount() -> CoreResult<String> {
        fromPrimitiveResult(result: export_account(documentsDirectory))
    }
    
    public func getUsage() -> CoreResult<[FileUsage]> {
        fromPrimitiveResult(result: get_usage(documentsDirectory))
    }
    
    public func synchronize() -> CoreResult<Empty> {
        fromPrimitiveResult(result: sync_all(documentsDirectory))
    }
    
    public func calculateWork() -> CoreResult<WorkMetadata> {
        fromPrimitiveResult(result: calculate_work(documentsDirectory))
    }
    
    public func executeWork(work: WorkUnit) -> CoreResult<Empty> {
        switch serialize(obj: work) {
        case .success(let workUnitStr):
            return fromPrimitiveResult(result: execute_work(documentsDirectory, workUnitStr))
        case .failure(let err):
            return CoreResult.failure(ApplicationError.General(err))
        }
    }
    
    public func setLastSynced(lastSync: UInt64) -> CoreResult<Empty> {
        fromPrimitiveResult(result: set_last_synced(documentsDirectory, lastSync))
    }
    
    public func getRoot() -> CoreResult<FileMetadata> {
        fromPrimitiveResult(result: get_root(documentsDirectory))
    }
    
    public func listFiles() -> CoreResult<[FileMetadata]> {
        fromPrimitiveResult(result: list_metadatas(documentsDirectory))
    }
    
    public func getFile(id: UUID) -> CoreResult<DecryptedValue> {
        fromPrimitiveResult(result: read_document(documentsDirectory, id.uuidString))
    }
    
    public func createFile(name: String, dirId: UUID, isFolder: Bool) -> CoreResult<FileMetadata> {
        let fileType = isFolder ? "Folder" : "Document"
        return fromPrimitiveResult(result: create_file(documentsDirectory, name, dirId.uuidString, fileType))
    }
    
    public func updateFile(id: UUID, content: String) -> CoreResult<Empty> {
        fromPrimitiveResult(result: write_document(documentsDirectory, id.uuidString, content))
    }
    
    public func markFileForDeletion(id: UUID) -> CoreResult<Bool> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    public func getApiLocation() -> String {
        let result = get_api_loc()
        let resultString = String(cString: result!)
        release_pointer(UnsafeMutablePointer(mutating: result))
        return resultString
    }
}


public struct FakeApi: LockbookApi {
    public init() {
        
    }
    
    public func getAccount() -> CoreResult<Account> {
        CoreResult.success(Account(username: username))
    }
    
    public func createAccount(username: String) -> CoreResult<Account> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    public func importAccount(accountString: String) -> CoreResult<Account> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    public func exportAccount() -> CoreResult<String> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    public func getUsage() -> CoreResult<[FileUsage]> {
        CoreResult.success([FileUsage(fileId: .init(), byteSecs: UInt64(100), secs: UInt64(1))])
    }
    
    public func synchronize() -> CoreResult<Empty> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    public func calculateWork() -> CoreResult<WorkMetadata> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    public func executeWork(work: WorkUnit) -> CoreResult<Empty> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    public func setLastSynced(lastSync: UInt64) -> CoreResult<Empty> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    public func getRoot() -> CoreResult<FileMetadata> {
        return CoreResult.success(FileMetadata(fileType: .Folder, id: root.id, parent: root.id, name: "first_file.md", owner: "root", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false))
    }
    
    public func listFiles() -> CoreResult<[FileMetadata]> {
        return Result.success(fileMetas)
    }
    
    public func getFile(id: UUID) -> CoreResult<DecryptedValue> {
        CoreResult.success(DecryptedValue(secret: """
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Morbi mattis mattis arcu a commodo. Maecenas dapibus mollis lacinia. Nunc ut mi felis. Donec efficitur, nulla venenatis sodales sagittis, elit tellus ullamcorper leo, in fringilla turpis nisl at sapien. Morbi et sagittis dolor, auctor sollicitudin lorem. In porttitor vulputate mi quis mattis. Suspendisse potenti. In leo sem, tincidunt ut diam sed, malesuada aliquet ipsum. Mauris pretium sapien non erat pulvinar, id dapibus dui convallis. Etiam maximus tellus ac nunc hendrerit vulputate. Vestibulum placerat ligula sit amet eleifend interdum. Pellentesque dignissim ipsum lectus, vitae ultricies mi accumsan id. Morbi ullamcorper gravida justo eu maximus. Pellentesque habitant morbi tristique senectus et netus et malesuada fames ac turpis egestas.

Nulla facilisi. Fusce ac risus ut sem vulputate euismod vitae ac massa. Quisque feugiat, risus in posuere varius, metus metus cursus lorem, at sollicitudin odio libero vel elit. Vestibulum ante ipsum primis in vel.
"""))
    }
    
    public func createFile(name: String, dirId: UUID, isFolder: Bool) -> CoreResult<FileMetadata> {
        let now = Date().timeIntervalSince1970
        return CoreResult.success(FileMetadata(fileType: .Document, id: UUID(uuidString: "c30a513a-0d75-4f10-ba1e-7a261ebbbe05").unsafelyUnwrapped, parent: dirId, name: "new_file.md", owner: username, contentVersion: UInt64(now), metadataVersion: UInt64(now), deleted: false))
    }
    
    public func updateFile(id: UUID, content: String) -> CoreResult<Empty> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    public func markFileForDeletion(id: UUID) -> CoreResult<Bool> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    public func getApiLocation() -> String {
        "fake://fake.lockbook.fake"
    }
    
    public let username: Account.Username = "jeff"
    public let root = FileMetadata(fileType: .Folder, id: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "jeff", owner: "jeff", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false)
    public let fileMetas = [
        FileMetadata(fileType: .Document, id: UUID(uuidString: "e956c7a2-db7f-4f9d-98c3-217847acf23a").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "first_file.md", owner: "jeff", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false),
        FileMetadata(fileType: .Document, id: UUID(uuidString: "644d1d56-8e24-4a32-8304-e906435f95db").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "another_file.md", owner: "jeff", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false),
        FileMetadata(fileType: .Document, id: UUID(uuidString: "c30a513a-0d75-4f10-ba1e-7a261ebbbe05").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "third_file.md", owner: "jeff", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false),
        FileMetadata(fileType: .Folder, id: UUID(uuidString: "cdcb3342-7373-4b11-96e9-eb25a703febb").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "nice_stuff", owner: "jeff", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false),
    ]
}
