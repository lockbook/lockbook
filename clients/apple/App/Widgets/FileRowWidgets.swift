import SwiftUI
import SwiftWorkspace

struct FileIcon: View {
    let file: File
    var systemName: String? = nil

    var body: some View {
        icon
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

extension FileIcon {
    @ViewBuilder
    private var icon: some View {
        let symbol = Image(systemName: systemName ?? FileIconHelper.fileToSystemImageName(file: file))
            .font(.system(size: 16))
        if systemName == nil {
            symbol.hidden().overlay {
                Text(FileIconHelper.fileToGlyph(file: file))
                    .font(IconFont.font(size: 16))
            }
        } else {
            symbol
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
