import Foundation

public enum DbState: String, Decodable {
    case ReadyToUse
    case Empty
    case MigrationRequired
    case StateRequiresClearing
}
