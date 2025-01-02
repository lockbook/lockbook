import SwiftUI

struct OptimizedSheetViewModifier<PresentedContent: View>: ViewModifier {
    @Binding var isPresented: Bool
    var width: CGFloat? = nil
    var height: CGFloat? = nil
    
    let presentedContent: () -> PresentedContent
    
    func body(content: Content) -> some View {
        if UIDevice.current.userInterfaceIdiom == .pad {
            if isPresented {
                content
                    .background(FormSheet(content: {
                        presentedContent()
                            .onDisappear {
                                isPresented = false
                            }
                            .frame(width: width, height: height)
                    }))
            } else {
                content
            }
        } else {
            content
                .sheet(isPresented: $isPresented, content: {
                    presentedContent()
                })
        }
    }
}

extension View {
    func optimizedSheet<PresentedContent: View>(isPresented: Binding<Bool>, width: CGFloat? = nil, height: CGFloat? = nil, presentedContent: @escaping () -> PresentedContent) -> some View {
        modifier(OptimizedSheetViewModifier(isPresented: isPresented, width: width, height: height, presentedContent: presentedContent))
    }
}

class FormSheetHostingController<Content>: UIHostingController<Content>, UIPopoverPresentationControllerDelegate where Content : View {
    required init?(coder: NSCoder) {
        fatalError("")
    }
    
    init(root: Content) {
        super.init(rootView: root)
        view.sizeToFit()
        preferredContentSize = view.bounds.size
        modalPresentationStyle = .formSheet
        presentationController!.delegate = self
    }
}

class FormSheetViewController<Content: View>: UIViewController {
    var content: () -> Content
    private var hostVC: FormSheetHostingController<Content>
        
    required init?(coder: NSCoder) { fatalError("") }
    
    init(content: @escaping () -> Content) {
        self.content = content
        hostVC = FormSheetHostingController(root: content())
        
        super.init(nibName: nil, bundle: nil)
    }
    
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        
        if presentedViewController == nil {
            present(hostVC, animated: true)
        }
    }
}

struct FormSheet<Content: View> : UIViewControllerRepresentable {
    let content: () -> Content
    
    func makeUIViewController(context: UIViewControllerRepresentableContext<FormSheet<Content>>) -> FormSheetViewController<Content> {
        FormSheetViewController(content: content)
    }
    
    func updateUIViewController(_ uiViewController: FormSheetViewController<Content>, context: UIViewControllerRepresentableContext<FormSheet<Content>>) {}
}

struct FormSheetViewModifier<ViewContent: View>: ViewModifier {
    @Binding var show: Bool
    
    let sheetContent: () -> ViewContent
    
    func body(content: Content) -> some View {
        if show {
            content
                .background(FormSheet(content: {
                    sheetContent()
                        .onDisappear {
                            show = false
                        }
                }))
        } else {
            content
        }
    }
}
