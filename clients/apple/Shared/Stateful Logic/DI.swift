import SwiftUI
import SwiftLockbookCore

class DI {
    static let core = CoreApi(documentsDirectory: ConfigHelper.getEnv(.lockbookLocation) ?? ConfigHelper.location)
    
    static let coreService: CoreService = CoreService(core)
    static let errors: UnexpectedErrorService = UnexpectedErrorService()
    static let accounts = AccountService(core)
    static let dbState: DbStateService = DbStateService(core)
    static let settings = SettingsService(core)
    static let status = StatusService(core)
    static let files = FileService(core)
    static let sync = SyncService(core)
    static let onboarding = OnboardingService(core)
    static let documentLoader = DocumentLoader(core)
    #if os(iOS)
    static let toolbarModel = ToolbarModel()
    #endif
}

class Mock {
    static let core = FakeApi()
    
    // Copy and Paste from above
    static let coreService: CoreService = CoreService(core)
    static let errors: UnexpectedErrorService = UnexpectedErrorService()
    static let accounts = AccountService(core)
    static let dbState: DbStateService = DbStateService(core)
    static let settings = SettingsService(core)
    static let status = StatusService(core)
    static let files = FileService(core)
    static let sync = SyncService(core)
    static let onboarding = OnboardingService(core)
    static let documentLoader = DocumentLoader(core)
    #if os(iOS)
    static let toolbarModel = ToolbarModel()
    #endif
    
    static let dummyAccount = Account(username: "Parth", apiUrl: "ftp://uranus.net", keys: [1, 2, 3, 4])
    static let dummyRoot = DecryptedFileMetadata(fileType: FileType.Folder, id: UUID.init(), parent: UUID.init(), decryptedName: "Parth", owner: "Parth", contentVersion: 0, metadataVersion: 0)
}

extension View {
    public func iOSDI() -> some View {
        #if os(iOS)
            return
                self
                    .environmentObject(DI.toolbarModel)
        #else
        return self
        #endif
    }
    public func realDI() -> some View {
        iOSDI()
            .environmentObject(DI.coreService)
            .environmentObject(DI.errors)
            .environmentObject(DI.accounts)
            .environmentObject(DI.dbState)
            .environmentObject(DI.settings)
            .environmentObject(DI.status)
            .environmentObject(DI.files)
            .environmentObject(DI.sync)
            .environmentObject(DI.onboarding)
            .environmentObject(DI.documentLoader)

    }
    
    public func mockiOSDI() -> some View {
        #if os(iOS)
            return
                self
                    .environmentObject(Mock.toolbarModel)
        #else
        return self
        #endif
    }
    
    public func mockDI() -> some View {
        mockiOSDI()
            .environmentObject(Mock.coreService)
            .environmentObject(Mock.errors)
            .environmentObject(Mock.accounts)
            .environmentObject(Mock.settings)
            .environmentObject(Mock.dbState)
            .environmentObject(Mock.status)
            .environmentObject(Mock.files)
            .environmentObject(Mock.sync)
            .environmentObject(Mock.onboarding)
            .environmentObject(Mock.documentLoader)
    }
}
