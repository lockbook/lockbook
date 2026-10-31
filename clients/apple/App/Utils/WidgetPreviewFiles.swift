import Foundation

/// Where the app keeps the widget's document previews: in the shared
/// container, each document's light and dark images beside the content
/// version they show.
enum WidgetPreviewFiles {
    /// The widget tiles' corner radius, which previews keep their content inside.
    static let cornerRadius: CGFloat = 9

    /// The narrowest tile's width: the medium widget's third on a 6.1" iPhone.
    static let tileWidth: CGFloat = 97

    static var directory: URL? {
        FileManager.default
            .containerURL(forSecurityApplicationGroupIdentifier: "group.app.lockbook")?
            .appendingPathComponent("previews", isDirectory: true)
    }

    static func image(of id: UUID, dark: Bool) -> URL {
        directory!.appendingPathComponent("\(id.uuidString)-\(dark ? "dark" : "light").png")
    }

    /// The content version the images show, as the app last rendered them.
    static func version(of id: UUID) -> String? {
        guard let url = directory?.appendingPathComponent("\(id.uuidString).version") else {
            return nil
        }
        return try? String(contentsOf: url, encoding: .utf8)
    }

    static func setVersion(_ version: String, of id: UUID) {
        guard let url = directory?.appendingPathComponent("\(id.uuidString).version") else { return }
        try? version.write(to: url, atomically: true, encoding: .utf8)
    }

    /// Removes the previews of documents not in `ids`.
    static func prune(keeping ids: Set<UUID>) {
        guard let directory,
              let names = try? FileManager.default.contentsOfDirectory(atPath: directory.path)
        else { return }
        for name in names {
            guard let id = UUID(uuidString: String(name.prefix(36))), !ids.contains(id) else {
                continue
            }
            try? FileManager.default.removeItem(at: directory.appendingPathComponent(name))
        }
    }
}
