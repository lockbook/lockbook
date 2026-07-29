import Foundation
import SwiftWorkspace

@Observable class RecentsModel {
    var previews: [UUID: DocPreview] = [:]

    let selection = SelectionModel()

    @ObservationIgnored private var loading: Set<UUID> = []

    func preview(for file: File) -> AttributedString? {
        previews[file.id]?.text
    }

    func loadPreviewIfNeeded(_ file: File) {
        guard let ext = file.name.split(separator: ".").last?.lowercased(),
              ext == "md" || Self.textExtensions.contains(ext),
              previews[file.id]?.lastModified != file.lastModified,
              !loading.contains(file.id)
        else {
            return
        }

        loading.insert(file.id)

        DispatchQueue.global(qos: .userInitiated).async {
            let text: AttributedString = switch AppState.lb.readDoc(id: file.id) {
            case let .success(bytes):
                Self.previewText(String(decoding: bytes, as: UTF8.self), markdown: ext == "md")
            case .failure:
                AttributedString()
            }

            DispatchQueue.main.async {
                self.loading.remove(file.id)
                self.previews[file.id] = DocPreview(lastModified: file.lastModified, text: text)
            }
        }
    }

    private static let textExtensions: Set<String> = [
        "txt", "log", "csv", "json", "xml", "html", "latex",
    ]

    static func previewText(_ content: String, markdown: Bool) -> AttributedString {
        var preview = AttributedString()

        for raw in content.prefix(4096).split(separator: "\n") {
            let trimmed = String(raw.trimmingCharacters(in: .whitespaces).prefix(200))

            guard !trimmed.isEmpty else {
                continue
            }

            var line = AttributedString(trimmed)

            if markdown,
               let parsed = try? AttributedString(
                   markdown: trimmed,
                   options: .init(interpretedSyntax: .full, failurePolicy: .returnPartiallyParsedIfPossible)
               )
            {
                guard !parsed.characters.isEmpty else {
                    continue
                }

                line = parsed
            }

            if !preview.characters.isEmpty {
                preview += AttributedString(" ")
            }

            preview += line

            if preview.characters.count >= 240 {
                break
            }
        }

        return preview
    }
}

struct DocPreview {
    let lastModified: UInt64
    let text: AttributedString
}
