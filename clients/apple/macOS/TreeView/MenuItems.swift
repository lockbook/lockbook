import AppKit
import SwiftLockbookCore

class CreateDocument: NSMenuItem {
    let file: File
    init(file: File) {
        self.file = file
        super.init(title: "Create document", action: #selector(create(_:)), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func create(_ sender: AnyObject) {
        DI.files.createDoc(maybeParent: file.id, isDrawing: false)
    }
}

class CreateFolder: NSMenuItem {
    let file: File
    init(file: File) {
        self.file = file
        super.init(title: "Create folder", action: #selector(create(_:)), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func create(_ sender: AnyObject) {
        DI.sheets.creatingFolderInfo = CreatingFolderInfo(parentPath: DI.files.getPathByIdOrParent(maybeId: file.id) ?? "ERROR", maybeParent: file.id)
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

class Share: NSMenuItem {
    let file: File
    init(file: File) {
        self.file = file
        super.init(title: "Share", action: #selector(share(_:)), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func share(_ sender: AnyObject) {
        DI.sheets.sharingFileInfo = file
    }
}

class CopyLink: NSMenuItem {
    let file: File
    
    init(file: File) {
        self.file = file
        super.init(title: "Copy file link", action: #selector(share(_:)), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func share(_ sender: AnyObject) {
        DI.files.copyFileLink(meta: file)
    }
}
