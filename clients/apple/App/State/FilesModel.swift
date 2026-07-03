import Foundation
import SwiftUI
import SwiftWorkspace

@Observable class FilesModel {
    var root: File? = nil
    var idsToFiles: [UUID: File] = [:]
    var childrenByParent: [UUID: [File]] = [:]

    var pendingSharesByUsername: [String: [File]]? = nil

    var statusDots: [UUID: SyncDot] = [:]

    @ObservationIgnored private let statusDotDelay: TimeInterval = 2
    @ObservationIgnored private var candidateStatusDots: [UUID: SyncDot] = [:]
    @ObservationIgnored private var statusDotSince: [UUID: Date] = [:]
    @ObservationIgnored private var statusDotTimer: Timer?

    init() {
        loadFiles()
    }

    func loadFiles() {
        DispatchQueue.global(qos: .userInitiated).async {
            guard let metas = self.value(of: AppState.lb.listMetadatas()),
                  let pendingShareFiles = self.value(of: AppState.lb.getPendingShareFiles()),
                  let pendingShares = self.value(of: AppState.lb.getPendingShares())
            else {
                return
            }

            var root: File? = nil
            var idsToFiles: [UUID: File] = [:]
            var childrenByParent: [UUID: [File]] = [:]

            func insert(_ file: File) {
                idsToFiles[file.id] = file

                if file.isRoot {
                    if root == nil {
                        root = file
                    }
                    return
                }

                if !childrenByParent[file.parent, default: []].contains(file) {
                    childrenByParent[file.parent, default: []].append(file)
                    childrenByParent[file.parent]?.sort()
                }
            }

            for file in metas {
                insert(file)
            }
            for file in pendingShareFiles {
                insert(file)
            }

            guard let root else {
                DispatchQueue.main.async {
                    AppState.shared.error = .custom(title: "Unexpected error", msg: "Could not find root")
                }
                return
            }

            var pendingSharesByUsername: [String: [File]] = [:]
            for file in pendingShares {
                guard let sharedBy = file.shareFrom(to: root.name) else {
                    continue
                }

                pendingSharesByUsername[sharedBy, default: []].append(file)
            }

            DispatchQueue.main.async {
                self.root = root
                self.idsToFiles = idsToFiles
                self.childrenByParent = childrenByParent
                self.pendingSharesByUsername = pendingSharesByUsername
                self.recomputeStatusDots(status: AppState.lb.events.status)
            }
        }
    }

    func canMove(_ file: File, into dest: File) -> Bool {
        guard dest.isFolder, dest.id != file.id, dest.id != file.parent else {
            return false
        }

        var current = dest
        while !current.isRoot {
            if current.id == file.id {
                return false
            }

            guard let parent = idsToFiles[current.parent] else {
                break
            }
            current = parent
        }

        return true
    }

    func moveFile(id: UUID, newParent: UUID) {
        mutateAndReload {
            AppState.lb.moveFile(id: id, newParent: newParent)
        }
    }

    func acceptShare(file: File) {
        guard let root else {
            return
        }

        mutateAndReload {
            AppState.lb.createLink(name: file.name, parent: root.id, target: file.id).map { _ in () }
        }
    }

    func rejectShare(id: UUID) {
        mutateAndReload {
            AppState.lb.deletePendingShare(id: id)
        }
    }

    private func mutateAndReload(_ operation: @escaping () -> Result<Void, LbError>) {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = operation()

            DispatchQueue.main.async {
                switch res {
                case .success:
                    self.loadFiles()
                case let .failure(err):
                    AppState.shared.error = .lb(error: err)
                }
            }
        }
    }

    private func value<T>(of result: Result<T, LbError>) -> T? {
        switch result {
        case let .success(value):
            return value
        case let .failure(err):
            DispatchQueue.main.async { AppState.shared.error = .lb(error: err) }
            return nil
        }
    }

    func recomputeStatusDots(status: Status) {
        candidateStatusDots = computeStatusDots(status: status)

        let now = Date()
        for id in candidateStatusDots.keys where statusDotSince[id] == nil {
            statusDotSince[id] = now
        }
        for id in statusDotSince.keys where candidateStatusDots[id] == nil {
            statusDotSince[id] = nil
        }

        refreshStatusDots()
    }

    private func refreshStatusDots() {
        statusDotTimer?.invalidate()
        statusDotTimer = nil

        let now = Date()
        var visible: [UUID: SyncDot] = [:]
        var nextDeadline: Date? = nil

        for (id, dot) in candidateStatusDots {
            guard let since = statusDotSince[id] else { continue }

            if now.timeIntervalSince(since) >= statusDotDelay {
                visible[id] = dot
            } else {
                let deadline = since.addingTimeInterval(statusDotDelay)
                if nextDeadline == nil || deadline < nextDeadline! {
                    nextDeadline = deadline
                }
            }
        }

        if visible != statusDots {
            statusDots = visible
        }

        if let nextDeadline {
            let interval = max(0, nextDeadline.timeIntervalSince(now))
            statusDotTimer = Timer.scheduledTimer(withTimeInterval: interval, repeats: false) { [weak self] _ in
                self?.refreshStatusDots()
            }
        }
    }

    private func computeStatusDots(status: Status) -> [UUID: SyncDot] {
        var dots: [UUID: SyncDot] = [:]

        func bump(_ id: UUID, _ dot: SyncDot) {
            if let existing = dots[id], existing.rank <= dot.rank {
                return
            }
            dots[id] = dot
        }

        func seed(_ ids: [UUID], _ dot: SyncDot) {
            for id in ids {
                bump(id, dot)

                var current = id
                while let file = idsToFiles[current], !file.isRoot {
                    guard let parent = idsToFiles[file.parent], !parent.isRoot else {
                        break
                    }
                    bump(parent.id, dot)
                    current = parent.id
                }
            }
        }

        seed(status.pushingFiles, .pushing)
        seed(status.dirtyLocally, .dirty)
        seed(status.pullingFiles, .pulling)

        return dots
    }
}

enum SyncDot: Equatable {
    case pushing
    case dirty
    case pulling

    var rank: Int {
        switch self {
        case .pushing: 0
        case .dirty: 1
        case .pulling: 2
        }
    }

    var color: Color {
        switch self {
        case .pushing: .green
        case .dirty: .yellow
        case .pulling: .blue
        }
    }
}

extension FilesModel {
    static var preview: FilesModel {
        FilesModel()
    }
}
