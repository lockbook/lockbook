import SwiftUI
import SwiftWorkspace

struct FileBreadcrumb: View {
    @Environment(FilesModel.self) private var filesModel

    let file: File
    var includeHome = false

    var body: some View {
        if let crumb = crumbText {
            crumb
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(.middle)
        }
    }

    private var segments: [String] {
        filesModel.ancestors(of: file).map(\.name)
    }

    private var crumbText: Text? {
        var names = segments
        var crumb: Text

        if includeHome {
            crumb = Text("\(Image(systemName: "house.fill")) Home")
        } else {
            guard let first = names.first else {
                return nil
            }

            crumb = Text("\(Image(systemName: "folder.fill")) \(first)")
            names.removeFirst()
        }

        for name in names {
            crumb = Text("\(crumb) \(Image(systemName: "chevron.compact.right")) \(name)")
        }

        return crumb
    }
}
