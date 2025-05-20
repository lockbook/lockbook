import SwiftUI

struct LargeNavigationTitleBar: ViewModifier {
    func body(content: Content) -> some View {
        #if os(iOS)
        content.navigationBarTitleDisplayMode(.large)
        #else
        content
        #endif
    }
}
