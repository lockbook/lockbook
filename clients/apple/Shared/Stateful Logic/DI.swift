import SwiftUI
import SwiftWorkspace

class DI {
    static let core = Lb(writablePath: ConfigHelper.getEnv(.lockbookLocation) ?? ConfigHelper.location, logs: true)

    static let coreService: CoreService = CoreService(core)
    static let errors: UnexpectedErrorService = UnexpectedErrorService()
    static let accounts = AccountService(core)
    static let settings = SettingsService(core)
    static let billing = BillingService(core)
    static let files = FileService(core)
    static let selected = SelectedFilesState()
    static let importExport = ImportExportService(core)
    static let sync = SyncService(core)
    static let share = ShareService(core)
    static let onboarding = OnboardingService(core)
    static let sheets: SheetState = SheetState()
    static let search = SearchService(core)
    static let workspace = WorkspaceState()
    
    public static func freshState() {
        DI.accounts.account = nil
        DI.settings.usages = nil
        DI.files.root = nil
        DI.files.idsAndFiles = [:]
        DI.settings.showView = false
        DI.onboarding.username = ""
    }
}

extension View {
    public func realDI() -> some View {
        self
            .environmentObject(DI.coreService)
            .environmentObject(DI.errors)
            .environmentObject(DI.accounts)
            .environmentObject(DI.settings)
            .environmentObject(DI.files)
            .environmentObject(DI.selected)
            .environmentObject(DI.importExport)
            .environmentObject(DI.sync)
            .environmentObject(DI.onboarding)
            .environmentObject(DI.sheets)
            .environmentObject(DI.billing)
            .environmentObject(DI.share)
            .environmentObject(DI.search)
            .environmentObject(DI.workspace)
    }
}
