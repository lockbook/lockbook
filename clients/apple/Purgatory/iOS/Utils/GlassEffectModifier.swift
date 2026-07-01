import SwiftUI

struct GlassEffectModifier: ViewModifier {
    func body(content: Content) -> some View {
        content
            .glassEffect(.regular)
    }
}
