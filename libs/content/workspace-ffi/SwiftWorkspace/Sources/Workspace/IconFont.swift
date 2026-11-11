import Bridge
import CoreText
import SwiftUI

public enum IconFont {
    private static let face: CGFont? = {
        guard let bytes = icon_font(),
              let provider = CGDataProvider(dataInfo: nil, data: bytes, size: Int(icon_font_len()), releaseData: { _, _, _ in })
        else { return nil }
        return CGFont(provider)
    }()

    public static func font(size: CGFloat) -> Font {
        guard let face else { return .system(size: size) }
        return Font(CTFontCreateWithGraphicsFont(face, size, nil, nil))
    }

    public static func glyph(fileName: String, isFolder: Bool) -> String {
        let scalar = fileName.withCString { file_row_icon($0, isFolder) }
        return Unicode.Scalar(scalar).map { String($0) } ?? ""
    }
}
