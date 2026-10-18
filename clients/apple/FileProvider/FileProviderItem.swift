import CryptoKit
import FileProvider
import UniformTypeIdentifiers

final class FileProviderItem: NSObject, NSFileProviderItem, Codable {
    let id: String
    let parent: String
    let name: String
    let folder: Bool
    let modified: UInt64
    var contentVersion: Data
    var size: Int?
    var writable: Bool

    init(id: String, parent: String, name: String, folder: Bool, modified: UInt64,
         contentVersion: Data, writable: Bool = true) {
        self.id = id
        self.parent = parent
        self.name = name
        self.folder = folder
        self.modified = modified
        self.contentVersion = contentVersion
        self.writable = writable
    }

    var itemIdentifier: NSFileProviderItemIdentifier { .init(id) }
    var parentItemIdentifier: NSFileProviderItemIdentifier { .init(parent) }
    var filename: String { name }
    var contentType: UTType { folder ? .folder : UTType(filenameExtension: (name as NSString).pathExtension) ?? .data }
    var documentSize: NSNumber? { size.map(NSNumber.init(value:)) }
    var contentModificationDate: Date? { Date(timeIntervalSince1970: Double(modified) / 1000) }
    var itemVersion: NSFileProviderItemVersion {
        let metadata = Data("\(parent)\u{0}\(name)\u{0}\(modified)\u{0}\(writable)\u{0}\(size ?? 0)".utf8)
        return NSFileProviderItemVersion(contentVersion: contentVersion, metadataVersion: Data(SHA256.hash(data: metadata)))
    }
    var capabilities: NSFileProviderItemCapabilities {
        var result: NSFileProviderItemCapabilities = [.allowsReading]
        if folder { result.insert(.allowsContentEnumerating) }
        if writable {
            result.formUnion(folder ? [.allowsAddingSubItems] : [.allowsWriting])
            if itemIdentifier != .rootContainer {
                result.formUnion([.allowsRenaming, .allowsReparenting, .allowsDeleting])
            }
        }
        return result
    }
}
