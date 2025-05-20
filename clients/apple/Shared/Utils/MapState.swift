import SwiftUI

struct MapStateViewModifier<T: Equatable>: ViewModifier {
    let from: Published<T>.Publisher
    @Binding var to: T

    func body(content: Content) -> some View {
        content
            .onReceive(from) { to = $0 }
    }
}

extension View {
    func mapState<T: Equatable>(_ from: Published<T>.Publisher, to: Binding<T>) -> some View {
        self.modifier(MapStateViewModifier(from: from, to: to))
    }
}
