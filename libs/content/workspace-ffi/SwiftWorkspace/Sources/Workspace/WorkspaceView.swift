import Bridge
import Combine
import Foundation
import MetalKit
import SwiftUI

#if os(iOS)
    import GameController
    import UIKit
    import ObjectiveC.runtime

    public struct WorkspaceView: UIViewControllerRepresentable {
        @EnvironmentObject public var workspaceInput: WorkspaceInputState
        @EnvironmentObject public var workspaceOutput: WorkspaceOutputState
        @Environment(\.horizontalSizeClass) var horizontalSizeClass

        public init() {}

        public func makeUIViewController(context: Context)
            -> ContainerController
        {
            return ContainerController(
                workspaceInput: workspaceInput,
                workspaceOutput: workspaceOutput
            )
        }

        public func updateUIViewController(
            _ uiViewController: ContainerController,
            context: Context
        ) {}

        // Tiny hosting container that manages adding/removing the preserved workspace instance
        public class ContainerController: UIViewController {
            let workspaceInput: WorkspaceInputState
            let workspaceOutput: WorkspaceOutputState

            init(
                workspaceInput: WorkspaceInputState,
                workspaceOutput: WorkspaceOutputState
            ) {
                self.workspaceInput = workspaceInput
                self.workspaceOutput = workspaceOutput

                super.init(nibName: nil, bundle: nil)
            }

            public required init?(coder: NSCoder) {
                fatalError("not supported")
            }

            public override func viewDidLoad() {
                let workspaceController: WorkspaceController

                if let wsHandle = workspaceInput.wsHandle {
                    workspaceController =
                        objc_getAssociatedObject(UIApplication.shared, wsHandle)
                        as! WorkspaceController
                } else {
                    let new = WorkspaceController(
                        workspaceInput: workspaceInput,
                        workspaceOutput: workspaceOutput
                    )
                    objc_setAssociatedObject(
                        UIApplication.shared,
                        workspaceInput.wsHandle!,
                        new,
                        .OBJC_ASSOCIATION_RETAIN_NONATOMIC
                    )
                    workspaceController = new
                }

                if workspaceController.parent != nil {
                    workspaceController.willMove(toParent: nil)
                    workspaceController.view.removeFromSuperview()
                    workspaceController.removeFromParent()
                }

                addChild(workspaceController)
                workspaceController.view.frame = view.bounds
                workspaceController.view.autoresizingMask = [
                    .flexibleWidth, .flexibleHeight,
                ]
                view.addSubview(workspaceController.view)
                workspaceController.didMove(toParent: self)

                workspaceController.inputManager.updateCurrentTab(
                    newCurrentTab: workspaceOutput.currentTab,
                    newTabCount: workspaceOutput.tabCount
                )
            }
        }
    }

    public class WorkspaceController: UIViewController {
        let inputManager: iOSMTKInputManager
        var cancellables: Set<AnyCancellable> = []

        init(
            workspaceInput: WorkspaceInputState,
            workspaceOutput: WorkspaceOutputState
        ) {
            self.inputManager = iOSMTKInputManager(
                workspaceInput,
                workspaceOutput
            )
            super.init(nibName: nil, bundle: nil)

            workspaceInput.redraw
                .sink { [weak self] _ in
                    DispatchQueue.main.async {
                        guard let self else { return }

                        self.inputManager.mtkView.setNeedsDisplay(
                            self.inputManager.mtkView.frame
                        )
                    }
                }
                .store(in: &cancellables)

            workspaceInput.focus
                .sink { [weak self] _ in
                    DispatchQueue.main.async {
                        guard let self else { return }

                        if let currentWrapper = self.inputManager
                            .currentWrapper,
                            currentWrapper.canBecomeFirstResponder
                        {
                            currentWrapper.becomeFirstResponder()
                        } else {
                            self.inputManager.mtkView.becomeFirstResponder()
                        }
                    }
                }
                .store(in: &cancellables)

            workspaceOutput
                .$currentTab
                .sink { [weak self] _ in
                    DispatchQueue.main.async {
                        guard let self else { return }

                        self.inputManager.updateCurrentTab(
                            newCurrentTab: workspaceOutput.currentTab,
                            newTabCount: workspaceOutput.tabCount
                        )
                    }
                }
                .store(in: &cancellables)

            view = inputManager
        }

        required init?(coder: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }
    }

    public class iOSMTKInputManager: UIView, UIGestureRecognizerDelegate {
        public var mtkView: iOSMTK

        var currentWrapper: UIView? = nil
        var tabCount: Int = 0

        init(
            _ workspaceInput: WorkspaceInputState,
            _ workspaceOutput: WorkspaceOutputState
        ) {
            mtkView = iOSMTK()
            mtkView.workspaceInput = workspaceInput
            mtkView.workspaceOutput = workspaceOutput

            mtkView.setInitialContent(workspaceInput.coreHandle)

            super.init(frame: .infinite)

            mtkView.translatesAutoresizingMaskIntoConstraints = false
            addSubview(mtkView)
            NSLayoutConstraint.activate([
                mtkView.topAnchor.constraint(equalTo: topAnchor),
                mtkView.leftAnchor.constraint(equalTo: leftAnchor),
                mtkView.rightAnchor.constraint(equalTo: rightAnchor),
                mtkView.bottomAnchor.constraint(equalTo: bottomAnchor),
            ])
        }

        required init?(coder aDecoder: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }

        public func updateCurrentTab(
            newCurrentTab: WorkspaceTab,
            newTabCount: Int
        ) {
            mtkView.tabSwitchTask = { [weak self] in

                guard let self else {
                    return
                }

                self.mtkView.onSelectionChanged = nil
                self.mtkView.onTextChanged = nil

                self.tabCount = newTabCount

                let headerSize = self.mtkView.docHeaderSize

                switch newCurrentTab {
                case .Welcome, .Pdf, .Loading, .SpaceInspector:
                    if self.currentWrapper == nil {
                        return
                    }

                    self.currentWrapper?.removeFromSuperview()
                    
                    self.currentWrapper = nil
                    self.mtkView.currentWrapper = nil
                case .Svg, .Image, .Graph:
                    if let currentWrapper = self.currentWrapper
                        as? SvgView,
                        currentWrapper.currentHeaderSize
                            == headerSize
                    {
                        self.mtkView.onTextChanged?()
                        return
                    }

                    self.currentWrapper?.removeFromSuperview()

                    let drawingWrapper = SvgView(
                        mtkView: self.mtkView,
                        headerSize: headerSize

                    )
                    self.currentWrapper = drawingWrapper
                    self.mtkView.currentWrapper = drawingWrapper

                    drawingWrapper
                        .translatesAutoresizingMaskIntoConstraints = false
                    self.addSubview(drawingWrapper)
                    NSLayoutConstraint.activate([
                        drawingWrapper.topAnchor.constraint(
                            equalTo: self.topAnchor,
                            constant: headerSize
                        ),
                        drawingWrapper.leftAnchor.constraint(
                            equalTo: self.leftAnchor
                        ),
                        drawingWrapper.rightAnchor.constraint(
                            equalTo: self.rightAnchor
                        ),
                        drawingWrapper.bottomAnchor.constraint(
                            equalTo: self.bottomAnchor
                        ),
                    ])
                case .PlainText, .Markdown:
                    if let currentWrapper = self.currentWrapper
                        as? MdView,
                        currentWrapper.currentHeaderSize
                            == headerSize
                    {
                        return
                    }

                    self.currentWrapper?.removeFromSuperview()

                    let textWrapper = MdView(
                        mtkView: self.mtkView,
                        headerSize: headerSize
                    )
                    self.currentWrapper = textWrapper
                    self.mtkView.currentWrapper = textWrapper

                    textWrapper.translatesAutoresizingMaskIntoConstraints =
                        false
                    self.addSubview(textWrapper)
                    NSLayoutConstraint.activate([
                        textWrapper.topAnchor.constraint(
                            equalTo: self.topAnchor,
                            constant: headerSize
                        ),
                        textWrapper.leftAnchor.constraint(
                            equalTo: self.leftAnchor
                        ),
                        textWrapper.rightAnchor.constraint(
                            equalTo: self.rightAnchor
                        ),
                        textWrapper.bottomAnchor.constraint(
                            equalTo: self.bottomAnchor,
                            constant: -MdView
                                .TOOL_BAR_HEIGHT
                        ),
                    ])

                    if GCKeyboard.coalesced != nil {
                        textWrapper.becomeFirstResponder()
                    }
                }
            }
        }
    }

#else
    public struct WorkspaceView: View, Equatable {
        @FocusState var focused: Bool
        @ObservedObject var workspaceInput: WorkspaceInputState
        @ObservedObject var workspaceOutput: WorkspaceOutputState

        let nsEditorView: NSWS

        public init(
            _ workspaceInput: WorkspaceInputState,
            _ workspaceOutput: WorkspaceOutputState,
            _ coreHandle: UnsafeMutableRawPointer?
        ) {
            self.workspaceInput = workspaceInput
            self.workspaceOutput = workspaceOutput

            nsEditorView = NSWS(workspaceInput, workspaceOutput, coreHandle)
        }

        public var body: some View {
            nsEditorView
                .focused($focused)
                .onAppear {
                    focused = true
                }
                .onReceive(
                    workspaceInput.focus,
                    perform: { newValue in
                        focused = true
                    }
                )
        }

        public static func == (lhs: WorkspaceView, rhs: WorkspaceView) -> Bool {
            true
        }
    }

    public struct NSWS: NSViewRepresentable {
        @ObservedObject public var workspaceInput: WorkspaceInputState
        @ObservedObject public var workspaceOutput: WorkspaceOutputState

        let coreHandle: UnsafeMutableRawPointer?

        public init(
            _ workspaceInput: WorkspaceInputState,
            _ workspaceOutput: WorkspaceOutputState,
            _ coreHandle: UnsafeMutableRawPointer?
        ) {
            self.workspaceInput = workspaceInput
            self.workspaceOutput = workspaceOutput
            self.coreHandle = coreHandle
        }

        public class Coordinator: NSObject {
            var cancellables: Set<AnyCancellable> = []
        }

        public func makeCoordinator() -> Coordinator {
            Coordinator()
        }

        public func makeNSView(context: NSViewRepresentableContext<NSWS>)
            -> MacMTK
        {
            let mtkView = MacMTK()
            mtkView.workspaceInput = workspaceInput
            mtkView.workspaceOutput = workspaceOutput
            mtkView.setInitialContent(coreHandle)

            workspaceInput.redraw
                .sink { _ in
                    mtkView.setNeedsDisplay(mtkView.frame)
                }
                .store(in: &context.coordinator.cancellables)

            return mtkView
        }

        public func updateNSView(
            _ nsView: MacMTK,
            context: NSViewRepresentableContext<NSWS>
        ) {}
    }
#endif
