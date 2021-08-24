import SwiftUI
import SwiftLockbookCore

class DI {
    static let core = CoreApi(documentsDirectory: ConfigHelper.getEnv(.lockbookLocation) ?? ConfigHelper.location)
    
    static let errors: UnexpectedErrorService = UnexpectedErrorService()
    static let accounts = AccountService(core)
    static let dbState: DbStateService = DbStateService(core)
    static let settings = SettingsService(core)
    static let openDrawing = DrawingModel(write: core.writeDrawing, read: core.readDrawing)
    static let openImage = ImageModel(read: core.exportDrawing)
    static let openDocument = Content(write: core.updateFile, read: core.getFile)
    static let status = StatusService(core)
    static let files = FileService(core)
    static let sync = SyncService(core)
    static let onboarding = OnboardingState(core)
    #if os(iOS)
    static let toolbarModel = ToolbarModel()
    #endif
}

class Mock {
    static let core = FakeApi()
    
    // Copy and Paste from above
    static let errors: UnexpectedErrorService = UnexpectedErrorService()
    static let accounts = AccountService(core)
    static let dbState: DbStateService = DbStateService(core)
    static let settings = SettingsService(core)
    static let openDrawing = DrawingModel(write: core.writeDrawing, read: core.readDrawing)
    static let openImage = ImageModel(read: core.exportDrawing)
    static let openDocument = Content(write: core.updateFile, read: core.getFile)
    static let status = StatusService(core)
    static let files = FileService(core)
    static let sync = SyncService(core)
    static let onboarding = OnboardingState(core)
    #if os(iOS)
    static let toolbarModel = ToolbarModel()
    #endif
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
            .environmentObject(DI.errors)
            .environmentObject(DI.accounts)
            .environmentObject(DI.dbState)
            .environmentObject(DI.settings)
            .environmentObject(DI.openDrawing)
            .environmentObject(DI.openImage)
            .environmentObject(DI.openDocument)
            .environmentObject(DI.status)
            .environmentObject(DI.files)
            .environmentObject(DI.sync)
            .environmentObject(DI.onboarding)

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
            .environmentObject(Mock.errors)
            .environmentObject(Mock.accounts)
            .environmentObject(Mock.settings)
            .environmentObject(Mock.dbState)
            .environmentObject(Mock.openDrawing)
            .environmentObject(Mock.openImage)
            .environmentObject(Mock.openDocument)
            .environmentObject(Mock.status)
            .environmentObject(Mock.files)
            .environmentObject(Mock.sync)
            .environmentObject(Mock.onboarding)
    }
}
