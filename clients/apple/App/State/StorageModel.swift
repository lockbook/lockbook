import Foundation
import SwiftWorkspace

enum StorageTab: String, CaseIterable, Identifiable {
    case folders = "Folders"
    case types = "Types"
    case largest = "Largest"
    case unsynced = "Unsynced"

    var id: Self { self }
}

@Observable class StorageModel {
    var usage: UsageMetrics? = nil
    var report: StorageReport? = nil
    var tab: StorageTab = .folders
    var folder: UUID? = nil

    func load(refreshCap: Bool = true) {
        DispatchQueue.global(qos: .userInitiated).async {
            let usage: UsageMetrics? = {
                if refreshCap, case let .success(usage) = AppState.lb.refreshUsage() {
                    return usage
                }
                return try? AppState.lb.getUsage().get()
            }()
            let entries = (try? AppState.lb.getUsageBreakdown().get()) ?? []
            let files = (try? AppState.lb.listMetadatas().get()) ?? []
            let report = files.first(where: { $0.id == $0.parent }).map {
                StorageReport(root: $0.id, files: files, entries: entries)
            }

            DispatchQueue.main.async {
                self.usage = usage ?? self.usage
                self.report = report ?? self.report
            }
        }
    }
}
