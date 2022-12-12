import Foundation

public struct WorkUnit: Decodable {
    public var content: File
    public var tag: String
}

public struct WorkCalculated: Decodable {
    public var mostRecentUpdateFromServer: UInt64
    public var workUnits: [WorkUnit]
}
