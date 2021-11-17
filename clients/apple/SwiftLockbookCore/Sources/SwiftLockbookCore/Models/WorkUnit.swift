import Foundation

public struct WorkMetadata: Decodable {
    public var mostRecentUpdateFromServer: Date
    public var localFiles: [DecryptedFileMetadata]
    public var serverFiles: [DecryptedFileMetadata]
    public var serverUnknownNameCount: Int
}
