import SwiftUI
import SwiftLockbookCore

class DI {
    static let core = CoreApi(ConfigHelper.getEnv(.lockbookLocation) ?? ConfigHelper.location, logs: true)

    static let coreService: CoreService = CoreService(core)
    static let errors: UnexpectedErrorService = UnexpectedErrorService()
    static let accounts = AccountService(core)
    static let settings = SettingsService(core)
    static let billing = BillingService(core)
    static let status = StatusService(core)
    static let files = FileService(core)
    static let sync = SyncService(core)
    static let share = ShareService(core)
    static let onboarding = OnboardingService(core)
    static let sheets: SheetState = SheetState()
    static let currentDoc: DocumentService = DocumentService()
    static let search = SearchService(core)
    #if os(iOS)
    static let toolbarModel = ToolbarModel()
    #endif
    
    public static func accountDeleted() {
        DI.accounts.account = nil
        DI.settings.usages = nil
        DI.files.root = nil
        DI.files.idsAndFiles = [:]
        DI.onboarding.theyChoseToBackup = false
        DI.onboarding.username = ""
        DI.currentDoc.openDocuments.removeAll()
    }
}

class Mock {
    static let core = FakeApi()
    
    // Copy and Paste from above
    static let coreService: CoreService = CoreService(core)
    static let errors: UnexpectedErrorService = UnexpectedErrorService()
    static let accounts = AccountService(core)
    static let settings = SettingsService(core)
    static let billing = BillingService(core)
    static let status = StatusService(core)
    static let files = FileService(core)
    static let sync = SyncService(core)
    static let share = ShareService(core)
    static let onboarding = OnboardingService(core)
    static let sheets: SheetState = SheetState()
    static let currentDoc: DocumentService = DocumentService()
    static let search = SearchService(core)
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
            .environmentObject(DI.coreService)
            .environmentObject(DI.errors)
            .environmentObject(DI.accounts)
            .environmentObject(DI.settings)
            .environmentObject(DI.status)
            .environmentObject(DI.files)
            .environmentObject(DI.sync)
            .environmentObject(DI.onboarding)
            .environmentObject(DI.sheets)
            .environmentObject(DI.currentDoc)
            .environmentObject(DI.billing)
            .environmentObject(DI.share)
            .environmentObject(DI.search)
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
            .environmentObject(Mock.status)
            .environmentObject(Mock.files)
            .environmentObject(Mock.sync)
            .environmentObject(Mock.onboarding)
            .environmentObject(Mock.sheets)
            .environmentObject(Mock.currentDoc)
            .environmentObject(Mock.billing)
            .environmentObject(Mock.share)
    }
}
