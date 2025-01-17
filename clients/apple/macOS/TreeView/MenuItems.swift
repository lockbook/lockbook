import AppKit
import SwiftWorkspace

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

class CreateDrawing: NSMenuItem {
    let file: File
    init(file: File) {
        self.file = file
        super.init(title: "Create drawing", action: #selector(create(_:)), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func create(_ sender: AnyObject) {
        DI.files.createDoc(maybeParent: file.id, isDrawing: true)
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

class RenameFile: NSMenuItem {
    let file: File
    init(file: File) {
        self.file = file
        super.init(title: "Rename", action: #selector(create(_:)), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func create(_ sender: AnyObject) {
        DI.sheets.renamingFileInfo = RenamingFileInfo(id: file.id, name: file.name, parentPath: DI.files.getPathByIdOrParent(maybeId: file.parent) ?? "ERROR")
    }
}

class Delete: NSMenuItem {
    let files: [File]
    init(files: [File]) {
        self.files = files
        super.init(title: "Delete", action: #selector(delete(_:)), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func delete(_ sender: AnyObject) {
        DI.sheets.deleteConfirmationInfo = files
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
        DI.files.copyFileLink(id: file.id)
    }
}

class ShareExternallyMenu: NSMenuItem {
    let file: File
    let fileTree: NSOutlineView

    init(file: File, fileTree: NSOutlineView) {
        self.file = file
        self.fileTree = fileTree

        super.init(title: "Share externally", action: nil, keyEquivalent: "")

        submenu = NSMenu(title: "Share externally")
        submenu!.addItem(ShareTo(file: file, fileTree: fileTree))
        
        if file.type == .document {
            submenu!.addItem(CopyLink(file: file))
        }
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}

class ShareTo: NSMenuItem {
    let file: File
    let fileTree: NSOutlineView

    init(file: File, fileTree: NSOutlineView) {
        self.file = file
        self.fileTree = fileTree
        super.init(title: "Share to...", action: #selector(create(_:)), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func create(_ sender: AnyObject) {
        if let dest = DI.importExport.exportFilesToTempDirSync(meta: file) {
            let maybeFileRow = fileTree.rowView(atRow: fileTree.row(forItem: file), makeIfNecessary: false)

            if let fileRow = maybeFileRow {
                NSSharingServicePicker(items: [dest]).show(relativeTo: .zero, of: fileRow, preferredEdge: .maxY)
            }
        }
    }
}

class ShareMultipleTo: NSMenuItem {
    let files: [File]
    let fileTree: NSOutlineView

    init(files: [File], fileTree: NSOutlineView) {
        self.files = files
        self.fileTree = fileTree
        super.init(title: "Share to...", action: #selector(create(_:)), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func create(_ sender: AnyObject) {
        var urls: [URL] = []
        
        for meta in files {
            if let url = DI.importExport.exportFilesToTempDirSync(meta: meta) {
                urls.append(url)
            }
        }
        
        NSSharingServicePicker(items: urls).show(relativeTo: .zero, of: fileTree, preferredEdge: .maxY)
    }
}
