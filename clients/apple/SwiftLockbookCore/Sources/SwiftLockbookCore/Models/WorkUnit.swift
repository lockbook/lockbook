import Foundation

public struct WorkMetadata: Decodable {
    public var mostRecentUpdateFromServer: Date
    public var localFiles: [ClientFileMetadata]
    public var serverFiles: [ClientFileMetadata]
    public var serverUnknownNameCount: Int
}
