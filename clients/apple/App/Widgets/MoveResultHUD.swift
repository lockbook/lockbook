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

struct MoveResultHUD: View {
    @Environment(FilesModel.self) private var filesModel

    let result: MoveResult

    private var fullSuccess: Bool {
        result.moved == result.requested
    }

    var body: some View {
        StatusHUD(
            message: message,
            systemImage: fullSuccess ? "checkmark" : "exclamationmark.triangle"
        )
        .task(id: result) {
            try? await Task.sleep(for: .seconds(1.8))

            withAnimation {
                filesModel.moveResult = nil
            }
        }
    }

    private var message: String {
        if result.requested == 1 {
            return result.moved == 1 ? "File moved" : "File not moved"
        }

        return "\(result.moved)/\(result.requested) files moved"
    }
}

struct ImportResultHUD: View {
    @Environment(FilesModel.self) private var filesModel

    let result: ImportResult

    var body: some View {
        StatusHUD(
            message: result.count == 1 ? "File imported" : "\(result.count) files imported",
            systemImage: "checkmark"
        )
        .task(id: result) {
            try? await Task.sleep(for: .seconds(1.8))

            withAnimation {
                filesModel.importResult = nil
            }
        }
    }
}

#Preview {
    VStack(spacing: 20) {
        MoveResultHUD(result: MoveResult(moved: 5, requested: 5))
        MoveResultHUD(result: MoveResult(moved: 2, requested: 5))
        MoveResultHUD(result: MoveResult(moved: 1, requested: 1))
        StatusHUD(message: "Opening link")
    }
    .frame(width: 400, height: 700)
    .environment(FilesModel.preview)
}
