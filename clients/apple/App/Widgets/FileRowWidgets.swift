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
