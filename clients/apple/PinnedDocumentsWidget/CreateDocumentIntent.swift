import AppIntents
import Bridge
import Foundation

struct WidgetOperationError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

func withWidgetCore<T>(_ operation: (OpaquePointer?) throws -> T) throws -> T {
    guard let directory = FileManager.default.containerURL(
        forSecurityApplicationGroupIdentifier: "group.app.lockbook"
    )?.appendingPathComponent("lockbook", isDirectory: true) else {
        throw WidgetOperationError(message: "Open Lockbook to set up your account.")
    }

    let core = lb_init(directory.path, "widgets", true, false)
    defer { lb_free_init(core) }
    if let error = core.err {
        throw WidgetOperationError(message: String(cString: error.pointee.msg))
    }
    return try operation(core.lb)
}

struct CreateDocumentIntent: AppIntent {
    static var title: LocalizedStringResource = "Create Document"
    static var isDiscoverable = false

    @Parameter(title: "Location") var folder: WidgetFolder
    @Parameter(title: "Drawing") var drawing: Bool

    init() {}

    init(folder: WidgetFolder, drawing: Bool = false) {
        self.folder = folder
        self.drawing = drawing
    }

    func perform() async throws -> some IntentResult & OpensIntent {
        let id = try withWidgetCore { core in
            let parent = LbUuid(bytes: folder.id.uuid)
            let file = lb_get_file(core, parent)
            defer { lb_free_file_res(file) }
            guard file.err == nil, file.file.typ.tag == LbFolder else {
                throw WidgetOperationError(message: "This location is unavailable. Choose another location in the widget's settings.")
            }

            let date = DateFormatter()
            date.locale = Locale(identifier: "en_US_POSIX")
            date.calendar = Calendar(identifier: .gregorian)
            date.dateFormat = "yyyy-MM-dd"
            let desired = "\(date.string(from: .now)).\(drawing ? "svg" : "md")"

            // Another widget may create a document between choosing a name and saving it.
            for _ in 0..<5 {
                let name = lb_next_name(core, parent, desired)
                defer { lb_free_next_name_res(name) }
                if let error = name.err {
                    throw WidgetOperationError(message: String(cString: error.pointee.msg))
                }

                let created = lb_create_file(core, name.name, parent, LbFileType())
                defer { lb_free_file_res(created) }
                if let error = created.err {
                    if error.pointee.code == PathTaken { continue }
                    throw WidgetOperationError(message: String(cString: error.pointee.msg))
                }
                return UUID(uuid: created.file.id.bytes)
            }
            throw WidgetOperationError(message: "Couldn't choose a new document name. Please try again.")
        }

        let url = URL(string: "https://app.lockbook.net/open/\(id.uuidString)")!
        return .result(opensIntent: OpenURLIntent(url))
    }
}
