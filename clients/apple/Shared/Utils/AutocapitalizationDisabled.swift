import SwiftUI

struct AutocapitalizationDisabled: ViewModifier {
    func body(content: Content) -> some View {
        #if os(iOS)
        content.textInputAutocapitalization(.never)
        #else
        content
        #endif
    }
}

extension View {
    func autocapitalizationDisabled() -> some View {
        modifier(AutocapitalizationDisabled())
    }
}
