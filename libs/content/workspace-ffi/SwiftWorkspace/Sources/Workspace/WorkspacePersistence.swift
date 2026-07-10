import Foundation

enum WorkspacePersistence {
    private static var activeWorkspaces = 0

    static func claim() -> Bool {
        activeWorkspaces += 1
        return activeWorkspaces == 1
    }

    static func release() {
        activeWorkspaces = max(0, activeWorkspaces - 1)
    }
}
