//
//  ShareViewController.swift
//  ShareExtension-iOS
//
//  Created by Smail Barkouch on 7/19/23.
//

import SwiftUI
import UIKit
import SwiftLockbookCore

class ShareViewController: UIViewController {
//    @IBOutlet var container: UIView!

    override func viewDidLoad() {
        super.viewDidLoad()
        
        if let item = extensionContext?.inputItems.first as? NSExtensionItem {
            if let attachments = item.attachments {
                for attachment: NSItemProvider in attachments {
                    if attachment.hasItemConformingToTypeIdentifier("public.url") {
                        attachment.loadItem(forTypeIdentifier: "public.url", options: nil, completionHandler: { text, _ in
                            DispatchQueue.main.async {
                                // text variable is the text from the share sheet...
                                let childView = UIHostingController(rootView: SubmitFileList())
                                self.addChild(childView)
                                childView.view.frame = self.view.bounds
                                self.view.addSubview(childView.view)
                                childView.didMove(toParent: self)
                            }
                        })
                    }
                }
            }
        }
        print("lb view loaded")
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        
        print("lb view appeared")
//        extensionContext?.completeRequest(returningItems: nil)
    }
}


struct SwiftUIView: View {
    @State public var incoming_text: String
    
    var body: some View {
        Text(incoming_text)
    }
}

struct SubmitFileList: View {
    let core: CoreApi
    let children: [File]

    @State var parent: File

    init() {
        self.core = CoreApi(FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.path, logs: true)
        self.children = try! core.listFiles().get()

        self._parent = State(initialValue: children.first(where: { meta in meta.id == meta.parent})!)
    }

    var body: some View {
        List {
            ForEach(children.filter({ meta in meta.parent == parent.id })) { meta in
                Text(meta.name)
            }
        }
    }
}
