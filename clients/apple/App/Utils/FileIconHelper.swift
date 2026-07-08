import SwiftWorkspace

enum FileIconHelper {
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

    static func docNameToSystemImageName(name: String) -> String {
        name.split(separator: ".").last.flatMap { extToSystemImg[String($0)] } ?? "doc"
    }

    static let extToSystemImg: [String: String] = [
        "md": "doc.richtext",
        "svg": "doc.text.image",
        "pdf": "doc.on.doc",
        "chat": "bubble.left",

        "txt": "doc.plaintext",
        "rtf": "doc.plaintext",
        "doc": "doc.plaintext",
        "docx": "doc.plaintext",

        "html": "chevron.left.slash.chevron.right",
        "xml": "chevron.left.slash.chevron.right",
        "json": "curlybraces",
        "latex": "sum",

        "png": "photo",
        "jpg": "photo",
        "jpeg": "photo",
        "tiff": "photo",
        "heif": "photo",
        "heic": "photo",
        "webp": "photo",

        "zip": "doc.zipper",
        "tar": "doc.zipper",
        "gz": "doc.zipper",
        "7z": "doc.zipper",
        "bz2": "doc.zipper",
        "xz": "doc.zipper",
        "iso": "doc.zipper",

        "log": "scroll",
        "csv": "tablecells",
    ]
}
