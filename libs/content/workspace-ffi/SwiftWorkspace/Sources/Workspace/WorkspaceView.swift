import Bridge
import Combine
import Foundation
import MetalKit
import SwiftUI

#if os(iOS)
    import GameController
    import ObjectiveC.runtime
    import UIKit

    public struct WorkspaceView: UIViewControllerRepresentable {
        @Environment(WorkspaceInputState.self) private var workspaceInput
        @Environment(WorkspaceOutputState.self) private var workspaceOutput
        @Environment(\.horizontalSizeClass) var horizontalSizeClass

        public init() {}

        public func makeUIViewController(context _: Context)
            -> ContainerController
        {
            ContainerController(
                workspaceInput: workspaceInput,
                workspaceOutput: workspaceOutput
            )
        }

        public func updateUIViewController(
            _: ContainerController,
            context _: Context
        ) {}

        /// Tiny hosting container that manages adding/removing the preserved workspace instance
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

            @available(*, unavailable)
            public required init?(coder _: NSCoder) {
                fatalError("not supported")
            }

            override public func viewDidLoad() {
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
                    newCurrentTab: workspaceOutput.currentTab
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
            inputManager = iOSMTKInputManager(
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

                        let manager = self.inputManager

                        if let currentWrapper = manager.currentWrapper,
                           currentWrapper.canBecomeFirstResponder,
                           currentWrapper.becomeFirstResponder()
                        {
                            return
                        }
                        manager.mtkView.becomeFirstResponder()
                    }
                }
                .store(in: &cancellables)

            inputManager.mtkView.currentTabChanged = { [weak self] currentTab in
                self?.inputManager.updateCurrentTab(newCurrentTab: currentTab)
            }

            view = inputManager
        }

        @available(*, unavailable)
        required init?(coder _: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }
    }

    public class iOSMTKInputManager: UIView, UIGestureRecognizerDelegate {
        public var mtkView: iOSMTK

        var currentWrapper: UIView?

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
                mtkView.bottomAnchor.constraint(
                    equalTo: bottomAnchor
                ),
            ])
        }

        @available(*, unavailable)
        required init?(coder _: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }

        public func updateCurrentTab(newCurrentTab: WorkspaceTab) {
            mtkView.tabSwitchTask = { [weak self] in
                guard let self else {
                    return
                }

                mtkView.onSelectionChanged = nil
                mtkView.onTextChanged = nil

                switch newCurrentTab {
                case .Welcome, .Pdf, .Loading, .SpaceInspector:
                    if currentWrapper == nil {
                        return
                    }

                    currentWrapper?.removeFromSuperview()

                    currentWrapper = nil
                    mtkView.currentWrapper = nil

                    mtkView.becomeFirstResponder()
                case .Svg, .Image, .Graph:
                    if currentWrapper is SvgView {
                        mtkView.onTextChanged?()
                        return
                    }

                    currentWrapper?.removeFromSuperview()

                    let drawingWrapper = SvgView(mtkView: mtkView)
                    currentWrapper = drawingWrapper
                    mtkView.currentWrapper = drawingWrapper

                    drawingWrapper
                        .translatesAutoresizingMaskIntoConstraints = false
                    addSubview(drawingWrapper)
                    NSLayoutConstraint.activate([
                        drawingWrapper.topAnchor.constraint(
                            equalTo: topAnchor
                        ),
                        drawingWrapper.leftAnchor.constraint(
                            equalTo: leftAnchor
                        ),
                        drawingWrapper.rightAnchor.constraint(
                            equalTo: rightAnchor
                        ),
                        drawingWrapper.bottomAnchor.constraint(
                            equalTo: bottomAnchor
                        ),
                    ])

                    mtkView.becomeFirstResponder()
                case .PlainText, .Markdown, .Chat:
                    if currentWrapper is MdView {
                        return
                    }

                    currentWrapper?.removeFromSuperview()

                    let textWrapper = MdView(mtkView: mtkView)
                    currentWrapper = textWrapper
                    mtkView.currentWrapper = textWrapper

                    // Frame-positioned from Rust's reported interaction rect
                    // each frame (see iOSMTKViewDelegate). Seed with the full
                    // view until the first rect arrives.
                    textWrapper.translatesAutoresizingMaskIntoConstraints = true
                    textWrapper.frame = mtkView.frame
                    addSubview(textWrapper)

                    if GCKeyboard.coalesced != nil {
                        textWrapper.becomeFirstResponder()
                    }
                }
            }
        }
    }

#else
    public struct WorkspaceView: NSViewRepresentable, Equatable {
        public let workspaceInput: WorkspaceInputState
        public let workspaceOutput: WorkspaceOutputState

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

        public func makeNSView(
            context: NSViewRepresentableContext<WorkspaceView>
        )
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

            workspaceInput.focus.sink(receiveValue: { _ in
                guard let window = mtkView.window else { return }
                window.makeFirstResponder(mtkView)
            })
            .store(in: &context.coordinator.cancellables)

            return mtkView
        }

        public func updateNSView(
            _: MacMTK,
            context _: NSViewRepresentableContext<WorkspaceView>
        ) {}

        public static func == (_: WorkspaceView, _: WorkspaceView) -> Bool {
            true
        }
    }
#endif
