import SwiftUI
import SwiftWorkspace

struct FileIcon: View {
    let file: File
    var systemName: String? = nil

    var body: some View {
        Image(systemName: systemName ?? FileIconHelper.fileToSystemImageName(file: file))
            .font(.system(size: 16))
            .frame(width: 16)
            .foregroundColor(file.isFolder ? .accentColor : .secondary)
            .overlay(alignment: .bottomTrailing) {
                if file.type == .document && !file.shares.isEmpty {
                    Image(systemName: "person.crop.circle.fill")
                        .font(.system(size: 9))
                        .foregroundColor(.accentColor)
                        .background(Circle().fill(.background))
                        .offset(x: 4, y: 3)
                }
            }
    }
}

struct DisclosureChevron: View {
    let isOpen: Bool

    var body: some View {
        Image(systemName: "chevron.forward")
            .renderingMode(.template)
            .resizable()
            .scaledToFit()
            .frame(width: 10, height: 10)
            .rotationEffect(.degrees(isOpen ? 90 : 0))
            .foregroundColor(.accentColor)
    }
}
