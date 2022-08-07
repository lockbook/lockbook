import Foundation

public struct WorkUnit: Decodable {
    public var content: File
    public var tag: String
}

public struct WorkMetadata: Decodable {
    public var mostRecentUpdateFromServer: Date
    public var workUnits: [WorkUnit]
}
