import SwiftUI

struct ActivityIndicator: View {
    @Environment(\.colorScheme) var colorScheme
    @Binding var status: Status

    var body: some View {
        ZStack {
            Rectangle()
                .foregroundColor(.textEditorBackground(isDark: colorScheme == .dark))
                .frame(width: 30, height: 30, alignment: .center)
                .cornerRadius(5)
                .opacity(0.9)
            Image(systemName: "externaldrive.fill.badge.checkmark")
                .foregroundColor(.green)
                .opacity(0.5)
        }
        .padding(.top, 2.0)
        .padding(.trailing, 20)
        .animation(.easeInOut(duration: 0.5))
        .onAppear(perform: {
            DispatchQueue.main.asyncAfter(deadline: .now() + 2, execute: {
                withAnimation {
                    status = .Inactive
                }
            })
        })
    }
}
