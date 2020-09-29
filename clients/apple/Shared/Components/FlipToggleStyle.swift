import SwiftUI

struct FlipToggleStyle: ToggleStyle {
    typealias Side = (String, systemImage: String, color: Color)
    let left: Side
    let right: Side
    
    func makeBody(configuration: Configuration) -> some View {
        Button(action: {
            configuration.isOn.toggle()
        }) {
            HStack {
                Label(left.0, systemImage: left.systemImage)
                    .foregroundColor(left.color)
                    .opacity(configuration.isOn ? 1 : 0.3)
                Text("/")
                    .foregroundColor(.black)
                Label(right.0, systemImage: right.systemImage)
                    .foregroundColor(right.color)
                    .opacity(configuration.isOn ? 0.3 : 1)
                
            }
        }
    }
}

struct FlipToggleStyle_Previews: PreviewProvider {
    static var previews: some View {
        Toggle("Folder", isOn: .constant(true))
            .toggleStyle(FlipToggleStyle(left: ("Doc", "doc", .pink), right: ("Folder", "folder", .purple)))
            .padding()
            .previewLayout(.sizeThatFits)
    }
}
