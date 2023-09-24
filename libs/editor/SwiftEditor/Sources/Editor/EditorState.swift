import SwiftUI
import Combine

public class EditorState: ObservableObject {
    
    @Published public var text: String
    @Published public var reloadText: Bool = false
    @Published public var reloadView: Bool = false
    @Published public var shouldFocus: Bool = true
    @Published public var pasted: Bool = false
    
    public var isiPhone: Bool
    
    public var importFile: (URL) -> String?
    
    public init(text: String, isiPhone: Bool, importFile: @escaping (URL) -> String?) {
        self.text = text
        self.isiPhone = isiPhone
        self.importFile = importFile
    }
}

public enum SupportedImportImageFormats {
    case png
    case tiff
}

public class ToolbarState: ObservableObject {
    @Published public var isBulletListSelected: Bool = false
    @Published public var isNumberListSelected: Bool = false
    @Published public var isTodoListSelected: Bool = false
    @Published public var isHeadingSelected: Bool = false
    @Published public var isInlineCodeSelected: Bool = false
    @Published public var isBoldSelected: Bool = false
    @Published public var isItalicSelected: Bool = false
    @Published public var isStrikethroughSelected: Bool = false
    
    public var toggleBulletList: () -> Void = {}
    public var toggleNumberList: () -> Void = {}
    public var toggleTodoList: () -> Void = {}
    public var toggleHeading: (UInt32) -> Void = {_ in }
    public var toggleInlineCode: () -> Void = {}
    public var toggleStrikethrough: () -> Void = {}
    public var toggleBold: () -> Void = {}
    public var toggleItalic: () -> Void = {}
    public var tab: (Bool) -> Void = {_ in }
    public var undoRedo: (Bool) -> Void = {_ in }
    
    public init() {}
}

public class NameState: ObservableObject {
    @Published public var potentialTitle: String? = nil
    
    public init() {}
}

func createTempDir() -> URL? {
    let fileManager = FileManager.default
    let tempTempURL = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("editor-tmp").appendingPathComponent(UUID().uuidString)
    
    do {
        try fileManager.createDirectory(at: tempTempURL, withIntermediateDirectories: true, attributes: nil)
    } catch {
        return nil
    }
    
    return tempTempURL
}

