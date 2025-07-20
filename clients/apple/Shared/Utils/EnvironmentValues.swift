import SwiftUI

extension EnvironmentValues {
    public var isPreview: Bool {
        get { self[isPreviewEnvironmentKey.self] }
        set {  }
    }
    
    public var isConstrainedLayout: Bool {
        get { self[isConstraintLayoutEnvironmentKey.self] }
        set { self[isConstraintLayoutEnvironmentKey.self] = newValue }
    }
}

struct isPreviewEnvironmentKey: EnvironmentKey {
    #if RELEASE
    static var defaultValue: Bool = false
    #else
    static var defaultValue: Bool = ProcessInfo.processInfo.environment["XCODE_RUNNING_FOR_PREVIEWS"] == "1"
    #endif
}

struct isConstraintLayoutEnvironmentKey: EnvironmentKey {
    #if os(iOS)
    static var defaultValue: Bool = UIDevice.current.userInterfaceIdiom == .phone
    #else
    static var defaultValue: Bool = false
    #endif
}
