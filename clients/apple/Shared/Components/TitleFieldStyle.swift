import SwiftUI

struct TitleFieldStyle: TextFieldStyle {
    @Environment(\.colorScheme) var colorScheme
    public func _body(configuration: TextField<Self._Label>) -> some View {
        let base = configuration
            .textFieldStyle(PlainTextFieldStyle())
            .font(.largeTitle)
            .multilineTextAlignment(.center)
            .border(Color.black, width: 0)
        #if os(macOS)
        return base
            .background(Color.textEditorBackground(isDark: colorScheme == .dark))
        #else
        return base
        #endif
    }
}

struct TitleFieldStyle_Previews: PreviewProvider {
    static var previews: some View {
        TextField("text...", text: .constant("text!"))
            .textFieldStyle(TitleFieldStyle())
            .padding()
            .previewLayout(.sizeThatFits)
    }
}
