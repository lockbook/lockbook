import AppKit
import SwiftLockbookCore

class Rename: NSMenuItem {
    let file: File
    init(file: File) {
        self.file = file
        super.init(title: "Rename", action: #selector(rename(_:)), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func rename(_ sender: AnyObject) {
        DI.sheets.renamingInfo = file
    }
}

class Create: NSMenuItem {
    let file: File
    init(file: File) {
        self.file = file
        super.init(title: "Create", action: #selector(create(_:)), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func create(_ sender: AnyObject) {
        DI.sheets.creatingInfo = CreatingInfo(parent: file, child_type: .Document)
    }
}

class Delete: NSMenuItem {
    let file: File
    init(file: File) {
        self.file = file
        super.init(title: "Delete", action: #selector(delete(_:)), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func delete(_ sender: AnyObject) {
        DI.files.deleteFile(id: file.id)
    }
}