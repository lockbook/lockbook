import SwiftUI

struct OptimizedSheetPresentingViewModifier<PresentedContent: View>: ViewModifier {
    @Environment(\.isConstrainedLayout) var isConstrainedLayout
    
    @Binding var isPresented: Bool
    @Binding var constrainedSheetHeight: CGFloat
    
    var width: CGFloat? = nil
    var height: CGFloat? = nil
    
    let presentedContent: () -> PresentedContent
    
    func body(content: Content) -> some View {
        if isConstrainedLayout {
            content
                .sheet(isPresented: $isPresented) {
                    presentedContent()
                        .autoSizeSheet(sheetHeight: $constrainedSheetHeight)
                }
        } else {
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
        }
    }
}

struct OptimizedSheetItemViewModifier<PresentedContent: View, Item: Identifiable>: ViewModifier {
    @Environment(\.isConstrainedLayout) var isConstrainedLayout
    
    @Binding var item: Item?
    @Binding var constrainedSheetHeight: CGFloat
    
    var width: CGFloat? = nil
    var height: CGFloat? = nil
    
    let presentedContent: (Item) -> PresentedContent
    
    func body(content: Content) -> some View {
        if isConstrainedLayout {
            content
                .sheet(item: $item) { item in
                    presentedContent(item)
                        .autoSizeSheet(sheetHeight: $constrainedSheetHeight)
                }
        } else {
            if let item {
                content
                    .background(FormSheet(content: {
                        presentedContent(item)
                            .onDisappear {
                                self.item = nil
                            }
                            .frame(width: width, height: height)
                    }))
            } else {
                content
            }
        }
    }
}

extension View {
    func optimizedSheet<PresentedContent: View>(isPresented: Binding<Bool>, constrainedSheetHeight: Binding<CGFloat>, width: CGFloat? = nil, height: CGFloat? = nil, presentedContent: @escaping () -> PresentedContent) -> some View {
        modifier(OptimizedSheetPresentingViewModifier(isPresented: isPresented, constrainedSheetHeight: constrainedSheetHeight, width: width, height: height, presentedContent: presentedContent))
    }
    
    func optimizedSheet<PresentedContent: View, Item: Identifiable>(item: Binding<Item?>, constrainedSheetHeight: Binding<CGFloat>, width: CGFloat? = nil, height: CGFloat? = nil, presentedContent: @escaping (Item) -> PresentedContent) -> some View {
        modifier(OptimizedSheetItemViewModifier(item: item, constrainedSheetHeight: constrainedSheetHeight, width: width, height: height, presentedContent: presentedContent))
    }
}

extension View {
    func autoSizeSheet(sheetHeight: Binding<CGFloat>) -> some View {
        modifier(AutoSizeSheetViewModifier(sheetHeight: sheetHeight))
    }
}

struct AutoSizeSheetViewModifier: ViewModifier {
    @Binding var sheetHeight: CGFloat
    
    func body(content: Content) -> some View {
        content
            .modifier(ReadHeightModifier())
            .onPreferenceChange(HeightPreferenceKey.self) { height in
                if let height {
                    self.sheetHeight = height
                }
            }
            .presentationDetents([.height(sheetHeight)])
            .presentationDragIndicator(.visible)

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

struct HeightPreferenceKey: PreferenceKey {
    static var defaultValue: CGFloat?

    static func reduce(value: inout CGFloat?, nextValue: () -> CGFloat?) {
        guard let nextValue = nextValue() else { return }
        value = nextValue
    }
}

struct ReadHeightModifier: ViewModifier {
    private var sizeView: some View {
        GeometryReader { geometry in
            Color.clear.preference(key: HeightPreferenceKey.self,
                value: geometry.size.height)
        }
    }

    func body(content: Content) -> some View {
        content.background(sizeView)
    }
}

