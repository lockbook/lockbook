import SwiftUI

struct StatusHUD: View {
    let message: String
    var systemImage: String? = nil

    var body: some View {
        VStack(spacing: 10) {
            if let systemImage {
                Image(systemName: systemImage)
                    .font(.system(size: 34, weight: .semibold))
                    .foregroundStyle(.secondary)
            } else {
                ProgressView()
                    .controlSize(.large)
            }

            Text(message)
                .font(.callout)
                .fontWeight(.medium)
        }
        .padding(30)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 16))
        .allowsHitTesting(false)
    }
}

struct ResultHUD<ID: Equatable>: View {
    let id: ID
    let message: String
    let systemImage: String
    let dismiss: () -> Void

    var body: some View {
        StatusHUD(message: message, systemImage: systemImage)
            .task(id: id) {
                try? await Task.sleep(for: .seconds(1.8))
                dismiss()
            }
    }
}

struct MoveResult: Equatable {
    let moved: Int
    let requested: Int

    var message: String {
        if requested == 1 {
            return moved == 1 ? "File moved" : "File not moved"
        }

        return "\(moved)/\(requested) files moved"
    }

    var systemImage: String {
        moved == requested ? "checkmark" : "exclamationmark.triangle"
    }
}

struct ImportResult: Equatable {
    let count: Int

    var message: String {
        count == 1 ? "File imported" : "\(count) files imported"
    }

    var systemImage: String {
        "checkmark"
    }
}

#Preview {
    let full = MoveResult(moved: 5, requested: 5)
    let partial = MoveResult(moved: 2, requested: 5)

    return VStack(spacing: 20) {
        ResultHUD(id: full, message: full.message, systemImage: full.systemImage) {}
        ResultHUD(id: partial, message: partial.message, systemImage: partial.systemImage) {}
        StatusHUD(message: "Opening link")
    }
    .frame(width: 400, height: 700)
}
