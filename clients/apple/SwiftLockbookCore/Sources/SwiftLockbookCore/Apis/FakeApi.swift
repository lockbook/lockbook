import Foundation

public struct FakeApi: LockbookApi {
    public init() {
        
    }
    
    public func getAccount() -> CoreResult<Account, GetAccountError> {
        CoreResult.success(.fake(username: username))
    }
    
    public func createAccount(username: String, apiLocation: String) -> CoreResult<Account, CreateAccountError> {
        CoreResult.failure(CoreError.lazy())
    }
    
    public func importAccount(accountString: String) -> CoreResult<Account, ImportError> {
        CoreResult.failure(CoreError.lazy())
    }
    
    public func exportAccount() -> CoreResult<String, AccountExportError> {
        CoreResult.failure(CoreError.lazy())
    }
    
    public func getUsage() -> CoreResult<[FileUsage], GetUsageError> {
        CoreResult.success([FileUsage(fileId: .init(), byteSecs: UInt64(100), secs: UInt64(1))])
    }
    
    public func synchronize() -> CoreResult<Empty, SyncAllError> {
        CoreResult.failure(CoreError.lazy())
    }
    
    public func calculateWork() -> CoreResult<WorkMetadata, CalculateWorkError> {
        CoreResult.failure(CoreError.lazy())
    }
    
    public func executeWork(work: WorkUnit) -> CoreResult<Empty, ExecuteWorkError> {
        CoreResult.failure(CoreError.lazy())
    }
    
    public func setLastSynced(lastSync: UInt64) -> CoreResult<Empty, SetLastSyncedError> {
        CoreResult.failure(CoreError.lazy())
    }
    
    public func getRoot() -> CoreResult<FileMetadata, GetRootError> {
        return CoreResult.success(FileMetadata(fileType: .Folder, id: root.id, parent: root.id, name: "first_file.md", owner: "root", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false))
    }
    
    public func listFiles() -> CoreResult<[FileMetadata], ListMetadatasError> {
        return CoreResult.success(fileMetas)
    }
    
    public func getFile(id: UUID) -> CoreResult<DecryptedValue, ReadDocumentError> {
        CoreResult.success(DecryptedValue(secret: """
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Morbi mattis mattis arcu a commodo. Maecenas dapibus mollis lacinia. Nunc ut mi felis. Donec efficitur, nulla venenatis sodales sagittis, elit tellus ullamcorper leo, in fringilla turpis nisl at sapien. Morbi et sagittis dolor, auctor sollicitudin lorem. In porttitor vulputate mi quis mattis. Suspendisse potenti. In leo sem, tincidunt ut diam sed, malesuada aliquet ipsum. Mauris pretium sapien non erat pulvinar, id dapibus dui convallis. Etiam maximus tellus ac nunc hendrerit vulputate. Vestibulum placerat ligula sit amet eleifend interdum. Pellentesque dignissim ipsum lectus, vitae ultricies mi accumsan id. Morbi ullamcorper gravida justo eu maximus. Pellentesque habitant morbi tristique senectus et netus et malesuada fames ac turpis egestas.

Nulla facilisi. Fusce ac risus ut sem vulputate euismod vitae ac massa. Quisque feugiat, risus in posuere varius, metus metus cursus lorem, at sollicitudin odio libero vel elit. Vestibulum ante ipsum primis in vel.
"""))
    }
    
    public func createFile(name: String, dirId: UUID, isFolder: Bool) -> CoreResult<FileMetadata, CreateFileError> {
        let now = Date().timeIntervalSince1970
        return CoreResult.success(FileMetadata(fileType: .Document, id: UUID(uuidString: "c30a513a-0d75-4f10-ba1e-7a261ebbbe05").unsafelyUnwrapped, parent: dirId, name: "new_file.md", owner: username, contentVersion: UInt64(now), metadataVersion: UInt64(now), deleted: false))
    }
    
    public func updateFile(id: UUID, content: String) -> CoreResult<Empty, WriteToDocumentError> {
        CoreResult.failure(CoreError.lazy())
    }
    
    public func markFileForDeletion(id: UUID) -> CoreResult<Bool, DeleteFileError> {
        CoreResult.failure(CoreError.lazy())
    }
    
    public func renameFile(id: UUID, name: String) -> CoreResult<Empty, RenameFileError> {
        CoreResult.failure(CoreError.lazy())
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
