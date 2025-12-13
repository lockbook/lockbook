import SwiftUI

struct GlassEffectModifier: ViewModifier {
    let radius: CGFloat = 20
    
    func body(content: Content) -> some View {
        if #available(iOS 26.0, *) {
            content
                .glassEffect(.regular)
        } else {
            content
        }
    }
}
