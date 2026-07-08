import SwiftUI

struct CollapsableSection<Label: View, Content: View>: View {
    @ViewBuilder var label: Label
    @ViewBuilder var content: Content

    @AppStorage var isOpen: Bool

    init(id: String, @ViewBuilder label: @escaping () -> Label, @ViewBuilder content: @escaping () -> Content) {
        self.label = label()
        self.content = content()
        _isOpen = AppStorage(wrappedValue: true, "CollapsableSection_\(id)")
    }

    var body: some View {
        Section(header: selectableLabel) {
            if isOpen {
                content
            }
        }
    }

    var selectableLabel: some View {
        Button(action: {
            withAnimation {
                isOpen.toggle()
            }
        }, label: {
            HStack(alignment: .lastTextBaseline) {
                label

                Spacer()

                Image(systemName: "chevron.right")
                    .foregroundColor(.secondary)
                    .padding(.leading, 5)
                    .imageScale(.small)
                    .rotationEffect(Angle(degrees: isOpen ? 90 : 0))
            }
            .contentShape(Rectangle())
        })
        .buttonStyle(.plain)
        .padding(.horizontal)
    }
}
