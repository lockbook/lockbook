import AppKit
import SwiftLockbookCore

class FileItemView: NSTableCellView {
    init(file: DecryptedFileMetadata) {
        let field = NSTextField(string: file.decryptedName)
        field.isEditable = false
        field.isSelectable = false
        field.isBezeled = false
        field.drawsBackground = false
        field.usesSingleLineMode = false
        field.cell?.wraps = false
        field.cell?.isScrollable = false

        super.init(frame: .zero)
        var imageView: NSImageView
        if file.fileType == .Document {
            let image = NSImage(systemSymbolName: "doc.fill", accessibilityDescription: nil)!
            image.isTemplate = true
            imageView = NSImageView(image: image)
            imageView.contentTintColor = .systemGray
        } else {
            let image = NSImage(systemSymbolName: "folder.fill", accessibilityDescription: nil)!
            image.isTemplate = true
            imageView = NSImageView(image: image)
            imageView.contentTintColor = .systemBlue
        }

        addSubview(imageView)
        addSubview(field)

        field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        field.translatesAutoresizingMaskIntoConstraints = false
        imageView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            field.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 20),
            field.trailingAnchor.constraint(equalTo: trailingAnchor),
            field.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            field.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
            imageView.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            imageView.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
        ])
    }
}
