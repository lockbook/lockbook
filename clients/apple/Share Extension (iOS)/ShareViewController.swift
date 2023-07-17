import SwiftUI
import SwiftLockbookCore
import UIKit

class ShareViewController: UIViewController {
    @IBOutlet var container: UIView!

    override func viewDidLoad() {
        super.viewDidLoad()

        // this gets the incoming information from the share sheet
        if let item = extensionContext?.inputItems.first as? NSExtensionItem {
            if let attachments = item.attachments {
                for attachment: NSItemProvider in attachments {
                    if attachment.hasItemConformingToTypeIdentifier("public.text") {
                        attachment.loadItem(forTypeIdentifier: "public.text", options: nil, completionHandler: { text, _ in

                            // text variable is the text from the share sheet...
                            let childView = UIHostingController(rootView: SwiftUIView(incoming_text: text as! String))
                            self.addChild(childView)
                            childView.view.frame = self.container.bounds
                            self.container.addSubview(childView.view)
                            childView.didMove(toParent: self)
                        })
                    }
                }
            }
        }
    }


    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
    }
    
    func close() {
        extensionContext?.completeRequest(returningItems: [], completionHandler: nil)
    }
}

struct SwiftUIView: View {
    @State public var incoming_text: String
    
    var body: some View {
        Text(incoming_text)
    }
}

//struct SubmitFileList: View {
//    let core: CoreApi
//    let children: [File]
//
//    @State var parent: File
//
//    init() {
//        self.core = CoreApi(FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.path, logs: true)
//        self.children = try! core.listFiles().get()
//
//        self._parent = State(initialValue: children.first(where: { meta in meta.id == meta.parent})!)
//    }
//
//    var body: some View {
//        List {
//            ForEach(children.filter({ meta in meta.parent == parent.id })) { meta in
//                Text(meta.name)
//            }
//        }
//    }
//}
//
//struct SubmitFileList: View {
//
//    var body: some View {
//        Text("COOkies are nice").font(.title)
//
//        List {
//            Text("Text 1")
//            Text("Text 2")
//            Text("Text 3")
//            Text("Text 4")
//            Text("Text 5")
//            Text("Text 6")
//            Text("Text 7")
//        }
//    }
//}
//
