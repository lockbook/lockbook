import SwiftUI

extension EnvironmentValues {
    var isPreview: Bool {
        self[isPreviewEnvironmentKey.self]
    }
}

struct isPreviewEnvironmentKey: EnvironmentKey {
    static var defaultValue: Bool = ProcessInfo.processInfo.environment["XCODE_RUNNING_FOR_PREVIEWS"] == "1"
}
