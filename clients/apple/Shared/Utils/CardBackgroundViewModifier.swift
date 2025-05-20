import SwiftUI

struct CardBackgroundViewModifier: ViewModifier {
    let background: Color
    
    func body(content: Content) -> some View {
        content
            .background(
                RoundedRectangle(cornerRadius: 5)
                    .fill(background)
                    .shadow(color: .black.opacity(0.2), radius: 4)
            )
            .padding(.vertical, 5)
    }
}

extension View {
    func cardBackground(background: Color) -> some View {
        modifier(CardBackgroundViewModifier(background: background))
    }
}
