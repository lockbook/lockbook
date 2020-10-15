import SwiftUI

struct TitleTextField: View {
    @Binding var text: String
    let doneEditing: () -> Void
    @Environment(\.colorScheme) var colorScheme
    
    var body: some View {
        let base = TextField("", text: $text, onEditingChanged: { editing in
            if !editing {
                doneEditing()
            }
        })
            .textFieldStyle(PlainTextFieldStyle())
            .font(.largeTitle)
            .multilineTextAlignment(.center)
            .border(Color.black, width: 0)
        #if os(macOS)
        return base
            .background(Color.textEditorBackground(isDark: colorScheme == .dark))
        #else
        return base
            .autocapitalization(.none)
        #endif
    }
}

struct TitleFieldStyle_Previews: PreviewProvider {
    static var previews: some View {
        TitleTextField(text: .constant("text!"), doneEditing: {})
            .padding()
            .previewLayout(.sizeThatFits)
    }
}
