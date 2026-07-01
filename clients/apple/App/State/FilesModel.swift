import Combine
import Foundation
import SwiftUI
import SwiftWorkspace

@Observable class FilesModel {
    var root: File? = nil
    var files: [File] = []
    var idsToFiles: [UUID: File] = [:]
    var childrens: [UUID: [File]] = [:]

    var pendingSharesByUsername: [String: [File]]? = nil

    var statusDots: [UUID: SyncDot] = [:]

    @ObservationIgnored private let statusDotDelay: TimeInterval = 2
    @ObservationIgnored private var candidateStatusDots: [UUID: SyncDot] = [:]
    @ObservationIgnored private var statusDotSince: [UUID: Date] = [:]
    @ObservationIgnored private var statusDotTimer: Timer?

    @ObservationIgnored private var cancellables: Set<AnyCancellable> = []

    init() {
        AppState.lb.events.$metadataUpdated.sink { [weak self] _ in
            self?.loadFiles()
        }
        .store(in: &cancellables)

        AppState.lb.events.$status.sink { [weak self] status in
            DispatchQueue.main.async {
                self?.recomputeStatusDots(status: status)
            }
        }
        .store(in: &cancellables)
    }

    func loadFiles() {
        DispatchQueue.global(qos: .userInitiated).async {
            let metas: [File]
            switch AppState.lb.listMetadatas() {
            case let .success(files):
                metas = files
            case let .failure(err):
                DispatchQueue.main.async { AppState.shared.error = .lb(error: err) }
                return
            }

            let pendingShareFiles: [File]
            switch AppState.lb.getPendingShareFiles() {
            case let .success(files):
                pendingShareFiles = files
            case let .failure(err):
                DispatchQueue.main.async { AppState.shared.error = .lb(error: err) }
                return
            }

            let pendingShares: [File]
            switch AppState.lb.getPendingShares() {
            case let .success(files):
                pendingShares = files
            case let .failure(err):
                DispatchQueue.main.async { AppState.shared.error = .lb(error: err) }
                return
            }

            var root: File? = nil
            var idsToFiles: [UUID: File] = [:]
            var childrens: [UUID: [File]] = [:]

            func insert(_ file: File) {
                idsToFiles[file.id] = file

                if childrens[file.parent] == nil {
                    childrens[file.parent] = []
                }

                if !file.isRoot {
                    if !childrens[file.parent]!.contains(file) {
                        childrens[file.parent]!.append(file)
                        childrens[file.parent]!.sort()
                    }
                } else if root == nil {
                    root = file
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
                self.files = metas
                self.idsToFiles = idsToFiles
                self.childrens = childrens
                self.pendingSharesByUsername = pendingSharesByUsername
                self.recomputeStatusDots(status: AppState.lb.events.status)
            }
        }
    }

    func acceptShare(file: File) {
        guard let root else {
            return
        }

        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.createLink(name: file.name, parent: root.id, target: file.id)

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

    func rejectShare(id: UUID) {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.deletePendingShare(id: id)

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

    private func recomputeStatusDots(status: Status) {
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
