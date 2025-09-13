import SwiftUI

extension EnvironmentValues {
    public var isPreview: Bool {
        get { self[isPreviewEnvironmentKey.self] }
        set {  }
    }
}

struct isPreviewEnvironmentKey: EnvironmentKey {
    #if RELEASE
    static var defaultValue: Bool = false
    #else
    static var defaultValue: Bool = ProcessInfo.processInfo.environment["XCODE_RUNNING_FOR_PREVIEWS"] == "1"
    #endif
}
