import SwiftUI

struct LargeNavigationTitle: ViewModifier {
    func body(content: Content) -> some View {
        #if os(iOS)
            content.navigationBarTitleDisplayMode(.large)
        #else
            content
        #endif
    }
}

extension View {
    func largeNavigationTitle() -> some View {
        modifier(LargeNavigationTitle())
    }
}
