import SwiftWorkspace

extension FileIconHelper {
    static func fileToSystemImageName(file: File) -> String {
        switch file.type {
        case .document:
            docNameToSystemImageName(name: file.name)
        case .folder:
            if file.shares.isEmpty {
                "folder.fill"
            } else {
                "folder.fill.badge.person.crop"
            }
        case .link:
            "folder.fill"
        }
    }

    static func fileToGlyph(file: File) -> String {
        switch file.type {
        case .document:
            docNameToGlyph(name: file.name)
        case .folder:
            file.shares.isEmpty ? Glyph.folder : Glyph.sharedFolder
        case .link:
            Glyph.folder
        }
    }
}
