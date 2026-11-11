enum FileIconHelper {
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

    static func docNameToGlyph(name: String) -> String {
        name.split(separator: ".").last.flatMap { extToGlyph[$0.lowercased()] } ?? Glyph.file
    }

    enum Glyph {
        static let folder = "\u{f024b}"
        static let sharedFolder = "\u{f024c}"
        static let file = "\u{e612}"
        static let markdown = "\u{e609}"
        static let drawing = "\u{e698}"
        static let pdf = "\u{e67d}"
        static let chat = "\u{f0b79}"
        static let text = "\u{e64e}"
        static let word = "\u{e6a5}"
        static let image = "\u{e60d}"
        static let video = "\u{e69f}"
        static let audio = "\u{e638}"
        static let archive = "\u{e6aa}"
        static let table = "\u{e64a}"
        static let log = "\u{f4ed}"
        static let html = "\u{e60e}"
        static let css = "\u{e614}"
        static let xml = "\u{e619}"
        static let json = "\u{f0626}"
        static let config = "\u{e615}"
        static let tex = "\u{e69b}"
        static let shell = "\u{e691}"
        static let rust = "\u{e68b}"
        static let python = "\u{e606}"
        static let javascript = "\u{e60c}"
        static let typescript = "\u{e628}"
        static let swift = "\u{e699}"
        static let kotlin = "\u{e634}"
        static let java = "\u{e66d}"
        static let c = "\u{e649}"
        static let cpp = "\u{e646}"
        static let go = "\u{e627}"
    }

    static let extToGlyph: [String: String] = [
        "md": Glyph.markdown,
        "svg": Glyph.drawing,
        "pdf": Glyph.pdf,
        "chat": Glyph.chat,

        "txt": Glyph.text,
        "rtf": Glyph.text,
        "doc": Glyph.word,
        "docx": Glyph.word,

        "png": Glyph.image,
        "jpg": Glyph.image,
        "jpeg": Glyph.image,
        "gif": Glyph.image,
        "tiff": Glyph.image,
        "heif": Glyph.image,
        "heic": Glyph.image,
        "webp": Glyph.image,
        "mp4": Glyph.video,
        "mov": Glyph.video,
        "mp3": Glyph.audio,
        "wav": Glyph.audio,

        "zip": Glyph.archive,
        "tar": Glyph.archive,
        "gz": Glyph.archive,
        "7z": Glyph.archive,
        "bz2": Glyph.archive,
        "xz": Glyph.archive,
        "iso": Glyph.archive,

        "log": Glyph.log,
        "csv": Glyph.table,

        "html": Glyph.html,
        "css": Glyph.css,
        "xml": Glyph.xml,
        "json": Glyph.json,
        "toml": Glyph.config,
        "yaml": Glyph.config,
        "yml": Glyph.config,
        "latex": Glyph.tex,
        "tex": Glyph.tex,
        "sh": Glyph.shell,
        "rs": Glyph.rust,
        "py": Glyph.python,
        "js": Glyph.javascript,
        "ts": Glyph.typescript,
        "swift": Glyph.swift,
        "kt": Glyph.kotlin,
        "java": Glyph.java,
        "c": Glyph.c,
        "h": Glyph.c,
        "cpp": Glyph.cpp,
        "go": Glyph.go,
    ]
}
