import SwiftUI
import SwiftLockbookCore

class DI {
    static let core = CoreApi(documentsDirectory: ConfigHelper.getEnv(.lockbookLocation) ?? ConfigHelper.location)
    
    static let errors: UnexpectedErrorService = UnexpectedErrorService()
    static let accounts = AccountService(core, errors)
    static let dbState: DbStateService = DbStateService(core, accounts, errors)
    static let settings = SettingsService(core, errors)
    static let openDrawing = DrawingModel(write: core.writeDrawing, read: core.readDrawing)
    static let toolbarModel = ToolbarModel()
    static let openImage = ImageModel(read: core.exportDrawing)
    static let openDocument = Content(write: core.updateFile, read: core.getFile)
    static let status = StatusService(core, accounts, errors)
    static let files = FileService(core, openDrawing, openImage, openDocument, errors)
    static let sync = SyncService(core, files, status, errors)
    static let onboarding = OnboardingState(core, accounts, files, errors)
}

class Mock {
    static let core = FakeApi()
    
    // Copy and Paste from above
    static let errors: UnexpectedErrorService = UnexpectedErrorService()
    static let accounts = AccountService(core, errors)
    static let dbState: DbStateService = DbStateService(core, accounts, errors)
    static let settings = SettingsService(core, errors)
    static let openDrawing = DrawingModel(write: core.writeDrawing, read: core.readDrawing)
    static let toolbarModel = ToolbarModel()
    static let openImage = ImageModel(read: core.exportDrawing)
    static let openDocument = Content(write: core.updateFile, read: core.getFile)
    static let status = StatusService(core, accounts, errors)
    static let files = FileService(core, openDrawing, openImage, openDocument, errors)
    static let sync = SyncService(core, files, status, errors)
    static let onboarding = OnboardingState(core, accounts, files, errors)
}

extension View {
    public func realDI() -> some View {
        self
            .environmentObject(DI.errors)
            .environmentObject(DI.accounts)
            .environmentObject(DI.dbState)
            .environmentObject(DI.settings)
            .environmentObject(DI.openDrawing)
            .environmentObject(DI.toolbarModel)
            .environmentObject(DI.openImage)
            .environmentObject(DI.openDocument)
            .environmentObject(DI.status)
            .environmentObject(DI.files)
            .environmentObject(DI.sync)
            .environmentObject(DI.onboarding)
    }
    
    public func mockDI() -> some View {
        self
            .environmentObject(Mock.errors)
            .environmentObject(Mock.accounts)
            .environmentObject(Mock.settings)
            .environmentObject(Mock.dbState)
            .environmentObject(Mock.openDrawing)
            .environmentObject(Mock.toolbarModel)
            .environmentObject(Mock.openImage)
            .environmentObject(Mock.openDocument)
            .environmentObject(Mock.status)
            .environmentObject(Mock.files)
            .environmentObject(Mock.sync)
            .environmentObject(Mock.onboarding)
    }
}
