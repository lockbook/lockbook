import Foundation

public struct FakeApi: LockbookApi {
    
    public init() {
    }
    
    public func getAccount() -> FfiResult<Account, GetAccountError> {
        .success(.fake(username: username))
    }
    
    public func createAccount(username: String, apiLocation: String, welcomeDoc: Bool) -> FfiResult<Empty, CreateAccountError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func importAccount(accountString: String) -> FfiResult<Empty, ImportError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func exportAccount() -> FfiResult<String, AccountExportError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func getUsage() -> FfiResult<UsageMetrics, GetUsageError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func getUncompressedUsage() -> FfiResult<UsageItemMetric, GetUsageError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func deleteAccount() -> FfiResult<Empty, DeleteAccountError> {
        .failure(.init(unexpected: "LAZY"))
    }

    public func syncAll() -> FfiResult<Empty, SyncAllError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func calculateWork() -> FfiResult<WorkCalculated, CalculateWorkError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func getRoot() -> FfiResult<File, GetRootError> {
        return .success(FakeApi.root)
    }
    
    public func listFiles() -> FfiResult<[File], ListMetadatasError> {
        return .success(FakeApi.fileMetas)
    }
    
    public func getFile(id: UUID) -> FfiResult<String, ReadDocumentError> {
        .success("""
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Morbi mattis mattis arcu a commodo.
Maecenas dapibus mollis lacinia. Nunc ut mi felis. Donec efficitur, nulla venenatis sodales sagittis, elit tellus ullamcorper leo, in fringilla turpis nisl at sapien.
Morbi et sagittis dolor, auctor sollicitudin lorem.
In porttitor vulputate mi quis mattis.
Suspendisse potenti. In leo sem, tincidunt ut diam sed, malesuada aliquet ipsum.
Mauris pretium sapien non erat pulvinar, id dapibus dui convallis. Etiam maximus tellus ac nunc hendrerit vulputate.
Vestibulum placerat ligula sit amet eleifend interdum.
Pellentesque dignissim ipsum lectus, vitae ultricies mi accumsan id.
Morbi ullamcorper gravida justo eu maximus.
Pellentesque habitant morbi tristique senectus et netus et malesuada fames ac turpis egestas.
Nulla facilisi.
Fusce ac risus ut sem vulputate euismod vitae ac massa.
Quisque feugiat, risus in posuere varius, metus metus cursus lorem, at sollicitudin odio libero vel elit.
Vestibulum ante ipsum primis in vel.
""")
    }
    
    public func readDrawing(id: UUID) -> FfiResult<Drawing, GetDrawingError> {
        .failure(.init(unexpected: "LAZY"))
    }
    public func writeDrawing(id: UUID, content: Drawing) -> FfiResult<Empty, WriteToDocumentError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func exportDrawing(id: UUID) -> FfiResult<Data, ExportDrawingError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func createFile(name: String, dirId: UUID, isFolder: Bool) -> FfiResult<File, CreateFileError> {
        let now = Date().timeIntervalSince1970
        return .success(File(fileType: .Document, id: UUID(uuidString: "c30a513a-0d75-4f10-ba1e-7a261ebbbe05").unsafelyUnwrapped, parent: dirId, name: "new_file.md", lastModifiedBy: username, lastModified: UInt64(now), shares: []))
    }
    
    public func createLink(name: String, dirId: UUID, target: UUID) -> FfiResult<Empty, CreateFileError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func updateFile(id: UUID, content: String) -> FfiResult<Empty, WriteToDocumentError> {
        .success(Empty())
    }
    
    public func deleteFile(id: UUID) -> FfiResult<Empty, FileDeleteError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func renameFile(id: UUID, name: String) -> FfiResult<Empty, RenameFileError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func moveFile(id: UUID, newParent: UUID) -> FfiResult<Empty, MoveFileError> {
        .failure(.init(unexpected: "LAZY"))
    }

    public func getLocalChanges() -> FfiResult<[UUID], GetLocalChangesError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func getLastSyncedHumanString() -> FfiResult<String, GetLastSyncedError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func newAppleSub(originalTransactionId: String, appAccountToken: String) -> FfiResult<Empty, UpgradeAccountAppStoreError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func cancelSub() -> FfiResult<Empty, CancelSubscriptionError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func shareFile(id: UUID, username: String, isWrite: Bool) -> FfiResult<Empty, ShareFileError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func getPendingShares() -> FfiResult<[File], GetPendingShares> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func deletePendingShare(id: UUID) ->FfiResult<Empty, DeletePendingShareError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func exportFile(id: UUID, destination: String) ->FfiResult<Empty, ExportFileError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func importFiles(sources: [String], destination: UUID) ->FfiResult<Empty, ImportFilesError> {
        .failure(.init(unexpected: "LAZY"))
    }

    public func getFileById(id: UUID) -> FfiResult<File, GetFileByIdError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func searchFilePaths(input: String) ->FfiResult<[SearchResultItem], SearchFilePathsError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func startSearch(context: UnsafeRawPointer?, updateStatus: @escaping @convention(c) (UnsafePointer<Int8>?, Int32, UnsafePointer<Int8>?) -> Void) -> FfiResult<Empty, GeneralSearchError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func endSearch() -> FfiResult<Empty, GeneralSearchError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func searchQuery(query: String) -> FfiResult<Empty, GeneralSearchError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func suggestedDocs() -> FfiResult<[UUID], SuggestedDocsError> {
        .failure(.init(unexpected: "LAZY"))
    }
    
    public func timeAgo(timeStamp: Int64) -> String {
        ""
    }
    
    public let username: Account.Username = "jeff"
    public static let root = File(fileType: .Folder, id: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "jeff", lastModifiedBy: "jeff",  lastModified: 1587384000000, shares: [])
    public static let fileMetas = [
        root,
        File(fileType: .Document, id: UUID(uuidString: "e956c7a2-db7f-4f9d-98c3-217847acf23a").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "first_file.md", lastModifiedBy: "jeff", lastModified: 1587384000000, shares: []),
        File(fileType: .Document, id: UUID(uuidString: "644d1d56-8e24-4a32-8304-e906435f95db").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "another_file.md", lastModifiedBy: "jeff", lastModified: 1587384000000, shares: []),
        File(fileType: .Document, id: UUID(uuidString: "c30a513a-0d75-4f10-ba1e-7a261ebbbe05").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "third_file.md", lastModifiedBy: "jeff", lastModified: 1587384000000, shares: []),
        File(fileType: .Folder, id: UUID(uuidString: "53470907-5628-49eb-a8b0-8212cf9c8a91").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "nice_stuff", lastModifiedBy: "jeff", lastModified: 1587384000000, shares: []),
        File(fileType: .Document, id: UUID(uuidString: "7578bedd-6429-47ad-b03d-a8d9eebaec0c").unsafelyUnwrapped, parent: UUID(uuidString: "53470907-5628-49eb-a8b0-8212cf9c8a91").unsafelyUnwrapped, name: "nice_1.txt", lastModifiedBy: "jeff", lastModified: 1587384000000, shares: []),
        File(fileType: .Document, id: UUID(uuidString: "c27e3c81-a2cb-4638-8ab5-c395bc119b92").unsafelyUnwrapped, parent: UUID(uuidString: "53470907-5628-49eb-a8b0-8212cf9c8a91").unsafelyUnwrapped, name: "nice_2.txt", lastModifiedBy: "jeff", lastModified: 1587384000000, shares: []),
        File(fileType: .Folder, id: UUID(uuidString: "7ca0d23a-4d17-478c-9152-c37683761ce2").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "other_stuff", lastModifiedBy: "jeff", lastModified: 1587384000000, shares: []),
        File(fileType: .Folder, id: UUID(uuidString: "5e828f37-4695-477d-8157-7434cea29474").unsafelyUnwrapped, parent: UUID(uuidString: "7ca0d23a-4d17-478c-9152-c37683761ce2").unsafelyUnwrapped, name: "deep_other_stuff", lastModifiedBy: "jeff", lastModified: 1587384000000, shares: []),
    ]
}
