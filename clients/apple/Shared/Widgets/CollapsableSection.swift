import SwiftUI

struct CollapsableSection<Label: View, Content: View>: View {
    let id: String
    @ViewBuilder var label: Label
    @ViewBuilder var content: Content
    
    @AppStorage var storedIsOpen: Bool
    @State private var isOpen: Bool
    
    init(id: String, @ViewBuilder label: @escaping () -> Label, @ViewBuilder content: @escaping () -> Content) {
        self.id = id
        self.label = label()
        self.content = content()
        let savedIsOpen = AppStorage(wrappedValue: true, "CollapsableSection_\(id)")
        self._storedIsOpen = savedIsOpen
        self._isOpen = State(initialValue: savedIsOpen.wrappedValue)
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
        .onChange(of: isOpen, perform: { newValue in
            storedIsOpen = isOpen
        })
    }
    
    var body: some View {
        Section(header: selectableLabel, content: {
            if isOpen {
                content
            }
        })
    }
}
