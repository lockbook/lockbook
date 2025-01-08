import SwiftUI

extension EnvironmentValues {
    public var isPreview: Bool {
        get { self[isPreviewEnvironmentKey.self] }
        set {  }
    }
    
    #if os(iOS)
    @Entry var isSidebarOpen: Bool = false
    @Entry var isConstrainedLayout: Bool = false
    #else
    
    #endif
}

struct isPreviewEnvironmentKey: EnvironmentKey {
    #if RELEASE
    static var defaultValue: Bool = false
    #else
    static var defaultValue: Bool = ProcessInfo.processInfo.environment["XCODE_RUNNING_FOR_PREVIEWS"] == "1"
    #endif
}

