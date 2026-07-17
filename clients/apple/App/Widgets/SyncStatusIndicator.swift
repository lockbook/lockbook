import SwiftUI
import SwiftWorkspace

enum SyncIndicator: Equatable {
    case synced
    case syncing
    case offline
    case outOfSpace
    case updateRequired
    case syncError

    init(status: Status) {
        if status.offline {
            self = .offline
        } else if status.outOfSpace {
            self = .outOfSpace
        } else if status.updateRequired {
            self = .updateRequired
        } else if status.unexpectedSyncProblem != nil {
            self = .syncError
        } else if status.syncing {
            self = .syncing
        } else {
            self = .synced
        }
    }

    var color: Color {
        switch self {
        case .synced: .green
        case .syncing: .blue
        case .offline: .gray
        case .outOfSpace: .orange
        case .updateRequired, .syncError: .red
        }
    }

    var shortLabel: String {
        switch self {
        case .synced: "Synced"
        case .syncing: "Syncing"
        case .offline: "Offline"
        case .outOfSpace: "No space"
        case .updateRequired: "Update"
        case .syncError: "Error"
        }
    }
}

enum UsageBarDisplayMode: String, Codable, CaseIterable, Identifiable {
    case always
    case never
    case whenHalf

    var id: Self { self }

    var label: String {
        switch self {
        case .always: "Always show"
        case .never: "Never show"
        case .whenHalf: "Show above 50%"
        }
    }
}

struct UsageBar: View {
    @AppStorage("usageBarMode") private var mode: UsageBarDisplayMode = .whenHalf

    private static let halfThreshold = 0.5
    private static let warnThreshold = 0.7

    var body: some View {
        if let usage = AppState.lb.events.status.spaceUsed, shouldShow(usage) {
            ProgressView(value: min(fraction(usage), 1))
                .tint(fraction(usage) >= Self.warnThreshold ? .yellow : .accentColor)
                .help("\(usage.serverUsedHuman) / \(usage.serverCapHuman)")
                .padding(.horizontal, 12)
                .padding(.top, 10)
        }
    }

    private func shouldShow(_ usage: UsageMetrics) -> Bool {
        switch mode {
        case .always: true
        case .never: false
        case .whenHalf: fraction(usage) >= Self.halfThreshold
        }
    }

    private func fraction(_ usage: UsageMetrics) -> Double {
        guard usage.serverCapExact > 0 else { return 0 }

        return Double(usage.serverUsedExact) / Double(usage.serverCapExact)
    }
}

struct SyncStatusDot: View {
    let indicator: SyncIndicator

    var body: some View {
        if indicator == .syncing {
            ProgressView()
                .controlSize(.small)
        } else {
            Circle()
                .fill(indicator.color)
                .frame(width: 8, height: 8)
        }
    }
}

struct SyncStatusFooter: View {
    let action: () -> Void

    @State private var stableMessage = ""
    @State private var spinAngle: Double = 0
    @State private var isSpinning = false

    var body: some View {
        let status = AppState.lb.events.status
        let raw = SyncIndicator(status: status)
        let indicator: SyncIndicator = raw == .syncing ? .synced : raw

        VStack(spacing: 0) {
            Divider()

            #if os(macOS)
                UsageBar()
            #endif

            Button(action: {
                if !isSpinning {
                    isSpinning = true
                    spinStep()
                }
                action()
            }) {
                HStack(spacing: 8) {
                    Circle()
                        .fill(indicator.color)
                        .frame(width: 8, height: 8)

                    Text(stableMessage.isEmpty ? indicator.shortLabel : stableMessage)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)

                    Spacer()

                    Image(systemName: "arrow.triangle.2.circlepath")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .rotationEffect(.degrees(spinAngle))
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(status.message)
        }
        .onChange(of: AppState.lb.events.status, initial: true) { _, status in
            if !status.syncing, !status.message.isEmpty {
                stableMessage = status.message
            }

            if status.syncing, !isSpinning {
                isSpinning = true
                spinStep()
            }
        }
    }

    private func spinStep() {
        withAnimation(.bouncy(duration: 0.50)) {
            spinAngle += 180
        } completion: {
            if AppState.lb.events.status.syncing {
                spinStep()
            } else {
                isSpinning = false
            }
        }
    }
}

enum SyncPillDisplay: Equatable {
    case syncing
    case synced
    case attention(SyncIndicator)

    static func attentionOnly(for status: Status) -> SyncIndicator? {
        switch SyncIndicator(status: status) {
        case .synced, .syncing: nil
        case let other: other
        }
    }

    private var indicator: SyncIndicator {
        switch self {
        case .syncing: .syncing
        case .synced: .synced
        case let .attention(indicator): indicator
        }
    }

    var label: String {
        indicator.shortLabel
    }

    var color: Color {
        indicator.color
    }

    var showsSpinner: Bool {
        self == .syncing
    }
}

struct SyncStatusPill: View {
    let display: SyncPillDisplay?
    let action: () -> Void

    var body: some View {
        Group {
            if let display {
                Button(action: action) {
                    HStack(spacing: 6) {
                        if display.showsSpinner {
                            ProgressView().controlSize(.small)
                        } else {
                            Circle().fill(display.color).frame(width: 8, height: 8)
                        }

                        Text(display.label)
                            .font(.footnote)
                            .fontWeight(.medium)
                    }
                }
                .tint(.primary)
                .transition(.scale(scale: 0.8).combined(with: .opacity))
            }
        }
    }
}

#Preview("Footer") {
    SyncStatusFooter {}
        .frame(width: 300)
}

#Preview("Pill") {
    SyncStatusPill(display: .attention(.offline)) {}
        .padding()
}
