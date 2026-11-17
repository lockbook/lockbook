import SwiftUI
import SwiftWorkspace

enum StorageInk {
    static let synced = Color.accentColor
    static let added = Color.yellow
    static let freed = Color.accentColor.opacity(0.3)
    static let over = Color.red
    static let track = Color.secondary.opacity(0.2)
}

func storageBytes(_ bytes: UInt64) -> String {
    UsageItem(exact: bytes).human
}

func storageSigned(_ delta: Int64) -> String {
    let human = storageBytes(delta.magnitude)
    return delta >= 0 ? "+\(human)" : "−\(human)"
}

func storageMessage(usage: UsageMetrics, outOfSpace: Bool) -> (title: String, detail: String)? {
    guard let cap = usage.dataCap else { return nil }
    let freeTier = Double(cap.exact) < BillingState.PREMIUM_DATA_CAP
    if usage.overCap {
        let over = storageBytes(usage.local.exact - cap.exact)
        return ("Over your data cap", "Changes that add space won't sync until you free \(over)\(freeTier ? " or upgrade" : "").")
    }
    if outOfSpace {
        return ("A sync ran out of space", "Your storage has room, so a folder someone shared with you may be full.")
    }
    if Double(usage.local.exact) >= Double(cap.exact) * 0.8 {
        return ("Almost full", "\(storageBytes(cap.exact - usage.local.exact)) left before changes stop syncing.")
    }
    return nil
}

struct StorageSummaryView: View {
    let usage: UsageMetrics

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Text(usage.local.human)
                    .font(.title.weight(.semibold))
                Text(usage.dataCap.map { "of \($0.human)" } ?? "used")
                    .foregroundStyle(.secondary)
                Spacer(minLength: 8)
                Text(side)
                    .foregroundStyle(usage.overCap ? StorageInk.over : .secondary)
            }

            StorageBar(usage: usage)
                .frame(height: 12)

            StorageLegend(usage: usage)
                .font(.subheadline)
        }
        .padding(.vertical, 6)
    }

    private var side: String {
        guard let cap = usage.dataCap else { return "Cap not loaded" }
        if usage.overCap {
            return "\(storageBytes(usage.local.exact - cap.exact)) over"
        }
        return "\(storageBytes(cap.exact - usage.local.exact)) free"
    }
}

struct StorageBar: View {
    let usage: UsageMetrics

    var body: some View {
        GeometryReader { geo in
            let synced = Double(usage.synced.exact)
            let local = Double(usage.local.exact)
            let added = Double(usage.added.exact)
            let freed = Double(usage.freed.exact)
            let cap = usage.dataCap.map { Double($0.exact) }
            let scale = max(cap ?? 0, synced + added, 1)
            let width = geo.size.width
            let height = geo.size.height
            let x = { (value: Double) -> CGFloat in width * CGFloat(min(max(value / scale, 0), 1)) }
            let kept = local - added
            let drawn = max(x(local + freed), height)

            ZStack(alignment: .leading) {
                Capsule().fill(StorageInk.track)

                ZStack(alignment: .leading) {
                    segment(x(0), x(kept), StorageInk.synced)
                    segment(x(kept), x(local), StorageInk.added)
                    segment(x(local), x(local + freed), StorageInk.freed)
                    if let cap, local > cap {
                        segment(x(cap), x(local), StorageInk.over)
                    }
                }
                .frame(width: width, height: height, alignment: .leading)
                .mask(alignment: .leading) {
                    Capsule().frame(width: drawn, height: height)
                }

                if let cap, scale > cap {
                    Rectangle()
                        .fill(Color.primary)
                        .frame(width: 1.5, height: height + 6)
                        .offset(x: x(cap) - 0.75)
                }
            }
        }
    }

    private func segment(_ from: CGFloat, _ to: CGFloat, _ color: Color) -> some View {
        Rectangle()
            .fill(color)
            .frame(width: max(to - from, 0))
            .offset(x: from)
    }
}

struct StorageLegend: View {
    let usage: UsageMetrics

    var body: some View {
        FlowLayout(spacing: 14, lineSpacing: 6) {
            item(StorageInk.synced, "Synced", usage.synced.human)
            if usage.added.exact > 0 {
                item(StorageInk.added, "Unsynced", "+\(usage.added.human)")
            }
            if usage.freed.exact > 0 {
                item(StorageInk.freed, "Freed on sync", "−\(usage.freed.human)")
            }
            if let cap = usage.dataCap, usage.overCap {
                item(StorageInk.over, "Over cap", storageBytes(usage.local.exact - cap.exact))
            }
        }
    }

    private func item(_ color: Color, _ label: String, _ value: String) -> some View {
        HStack(spacing: 5) {
            RoundedRectangle(cornerRadius: 2.5)
                .fill(color)
                .frame(width: 9, height: 9)
            Text(label)
                .foregroundStyle(.secondary)
            Text(value)
        }
    }
}

struct FlowLayout: Layout {
    var spacing: CGFloat
    var lineSpacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache _: inout ()) -> CGSize {
        let rows = rows(width: proposal.width ?? .infinity, subviews: subviews)
        let width = rows.map(\.width).max() ?? 0
        let height = rows.map(\.height).reduce(0, +) + lineSpacing * CGFloat(max(rows.count - 1, 0))
        return CGSize(width: proposal.width ?? width, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal _: ProposedViewSize, subviews: Subviews, cache _: inout ()) {
        var y = bounds.minY
        for row in rows(width: bounds.width, subviews: subviews) {
            var x = bounds.minX
            for index in row.indices {
                let size = subviews[index].sizeThatFits(.unspecified)
                subviews[index].place(at: CGPoint(x: x, y: y + (row.height - size.height) / 2), proposal: .unspecified)
                x += size.width + spacing
            }
            y += row.height + lineSpacing
        }
    }

    private func rows(width: CGFloat, subviews: Subviews) -> [(indices: [Int], width: CGFloat, height: CGFloat)] {
        var rows: [(indices: [Int], width: CGFloat, height: CGFloat)] = []
        var current: (indices: [Int], width: CGFloat, height: CGFloat) = ([], 0, 0)
        for index in subviews.indices {
            let size = subviews[index].sizeThatFits(.unspecified)
            let next = current.indices.isEmpty ? size.width : current.width + spacing + size.width
            if !current.indices.isEmpty, next > width {
                rows.append(current)
                current = ([], 0, 0)
            }
            current.width = current.indices.isEmpty ? size.width : current.width + spacing + size.width
            current.height = max(current.height, size.height)
            current.indices.append(index)
        }
        if !current.indices.isEmpty {
            rows.append(current)
        }
        return rows
    }
}

struct StorageRowView: View {
    let systemImage: String
    let title: String
    var detail: String? = nil
    var fraction: Double? = nil
    var barColor: Color = StorageInk.synced
    let value: String
    var delta: (text: String, color: Color)? = nil
    var chevron: Bool? = nil

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: systemImage)
                .foregroundStyle(.secondary)
                .frame(width: 22)

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .lineLimit(1)
                    .truncationMode(.middle)
                if let detail {
                    Text(detail)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }

            Spacer(minLength: 8)

            if let fraction {
                ZStack(alignment: .leading) {
                    Capsule().fill(StorageInk.track)
                    Capsule()
                        .fill(barColor)
                        .frame(width: max(48 * min(max(fraction, 0), 1), 5))
                }
                .frame(width: 48, height: 5)
            }

            VStack(alignment: .trailing, spacing: 2) {
                Text(value)
                    .monospacedDigit()
                if let delta {
                    Text(delta.text)
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(delta.color)
                }
            }
            .frame(width: 84, alignment: .trailing)

            if let chevron {
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.tertiary)
                    .opacity(chevron ? 1 : 0)
            }
        }
        .contentShape(Rectangle())
    }
}

struct StorageBreakdownRows: View {
    @Bindable var model: StorageModel
    let report: StorageReport

    var body: some View {
        switch model.tab {
        case .folders: folders
        case .types: types
        case .largest: largest
        case .unsynced: unsynced
        }
    }

    @ViewBuilder private var folders: some View {
        let focus = model.folder.flatMap { report.isFolder($0) ? $0 : nil } ?? report.root
        let rows = report.children(of: focus)

        if focus != report.root {
            let trail = report.trail(to: focus)
            let parent = trail.count > 1 ? trail[trail.count - 2].id : report.root
            let size = report.row(focus)?.size ?? StorageSize()
            Button {
                model.folder = parent
            } label: {
                StorageRowView(
                    systemImage: "arrow.left",
                    title: trail.dropFirst().map(\.name).joined(separator: " / "),
                    detail: "Back to the folder above",
                    value: storageBytes(size.local),
                    delta: delta(size)
                )
            }
            .buttonStyle(.plain)
        }

        if rows.isEmpty {
            empty("Nothing stored here yet.")
        }

        let max = rows.map(\.size.local).max() ?? 1
        let hasFolders = rows.contains(where: \.isFolder)
        ForEach(rows) { row in
            if row.isFolder {
                Button {
                    model.folder = row.id
                } label: {
                    fileRow(row, detail: items(row.count - 1), max: max, chevron: true)
                }
                .buttonStyle(.plain)
                .contextMenu {
                    Button("Locate in File Tree", systemImage: "scope") {
                        show(row.id)
                    }
                }
            } else {
                showable(row.id, isFolder: false) {
                    fileRow(row, detail: nil, max: max, chevron: hasFolders ? false : nil)
                }
            }
        }
    }

    @ViewBuilder private var types: some View {
        if report.kinds.isEmpty {
            empty("Nothing stored yet.")
        }
        let max = report.kinds.map(\.size.local).max() ?? 1
        ForEach(report.kinds) { row in
            StorageRowView(
                systemImage: row.kind.systemImage,
                title: row.kind.label,
                detail: plural(row.count, row.kind == .folders ? "item" : "file"),
                fraction: Double(row.size.local) / Double(Swift.max(max, 1)),
                value: storageBytes(row.size.local),
                delta: delta(row.size)
            )
        }
    }

    @ViewBuilder private var largest: some View {
        if report.largest.isEmpty {
            empty("No documents yet.")
        }
        let max = report.largest.first?.size.local ?? 1
        ForEach(report.largest) { row in
            showable(row.id, isFolder: false) {
                fileRow(row, detail: "in \(report.parentPath(of: row.id))", max: max, chevron: nil)
            }
        }
    }

    @ViewBuilder private var unsynced: some View {
        if report.changes.isEmpty {
            empty("Everything is synced.")
        }
        let max = report.changes.map { $0.size.delta.magnitude }.max() ?? 1
        ForEach(report.changes) { row in
            if let id = row.fileId, row.change != .deleted {
                showable(id, isFolder: row.isFolder) {
                    changeRow(row, detail: "\(row.change.label) · in \(report.parentPath(of: id))", max: max)
                }
            } else {
                changeRow(row, detail: "Space returns once the deletes sync", max: max)
            }
        }
    }

    private func changeRow(_ row: StorageChangeRow, detail: String, max: UInt64) -> some View {
        StorageRowView(
            systemImage: row.change == .deleted ? "trash" : icon(row.name, row.isFolder),
            title: row.name,
            detail: detail,
            fraction: Double(row.size.delta.magnitude) / Double(Swift.max(max, 1)),
            barColor: row.size.delta > 0 ? StorageInk.added : StorageInk.freed,
            value: storageSigned(row.size.delta)
        )
    }

    private func showable(_ id: UUID, isFolder: Bool, @ViewBuilder label: () -> some View) -> some View {
        Button {
            show(id)
        } label: {
            label()
        }
        .buttonStyle(.plain)
        .help(isFolder ? "Locate in File Tree" : "Open")
    }

    private func show(_ id: UUID) {
        NotificationCenter.default.post(name: .showFromStorage, object: id)
    }

    private func fileRow(_ row: StorageRow, detail: String?, max: UInt64, chevron: Bool?) -> some View {
        StorageRowView(
            systemImage: icon(row.name, row.isFolder),
            title: row.name,
            detail: detail,
            fraction: Double(row.size.local) / Double(Swift.max(max, 1)),
            value: storageBytes(row.size.local),
            delta: delta(row.size),
            chevron: chevron
        )
    }

    private func empty(_ message: String) -> some View {
        Text(message)
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .center)
            .padding(.vertical, 12)
    }

    private func delta(_ size: StorageSize) -> (text: String, color: Color)? {
        switch size.delta {
        case 0: nil
        case let d where d > 0: (storageSigned(d), StorageInk.added)
        case let d: (storageSigned(d), .secondary)
        }
    }

    private func icon(_ name: String, _ isFolder: Bool) -> String {
        isFolder ? "folder" : FileIconHelper.docNameToSystemImageName(name: name)
    }

    private func items(_ n: Int) -> String {
        plural(n, "item")
    }

    private func plural(_ n: Int, _ noun: String) -> String {
        n == 1 ? "1 \(noun)" : "\(n) \(noun)s"
    }
}
