import Foundation
import SwiftUI
import MetalKit
import Combine
import Bridge

#if os(iOS)
import GameController

public struct WorkspaceView: View, Equatable {
    
    let workspaceInput: WorkspaceInputState
    let workspaceOutput: WorkspaceOutputState
    
    let coreHandle: UnsafeMutableRawPointer?
        
    public init(_ workspaceInput: WorkspaceInputState, _ workspaceOutput: WorkspaceOutputState, _ coreHandle: UnsafeMutableRawPointer?) {
        self.workspaceInput = workspaceInput
        self.workspaceOutput = workspaceOutput
        
        self.coreHandle = coreHandle
    }
    
    public var body: some View {
        UIWS(workspaceInput, workspaceOutput, coreHandle)
    }
    
    public static func == (lhs: WorkspaceView, rhs: WorkspaceView) -> Bool {
        return true
    }
}

public struct UIWS: UIViewRepresentable {
    @ObservedObject public var workspaceInput: WorkspaceInputState
    @ObservedObject public var workspaceOutput: WorkspaceOutputState
    
    let coreHandle: UnsafeMutableRawPointer?
        
    var openDoc: UUID? = nil
            
    public init(_ workspaceInput: WorkspaceInputState, _ workspaceOutput: WorkspaceOutputState, _ coreHandle: UnsafeMutableRawPointer?) {
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

    public func makeUIView(context: Context) -> iOSMTKInputManager {
        let inputManager = iOSMTKInputManager(workspaceInput, workspaceOutput, coreHandle)
        
        workspaceInput.redraw
            .sink { _ in
                DispatchQueue.main.async {
                    inputManager.mtkView.setNeedsDisplay(inputManager.mtkView.frame)
                }
            }
            .store(in: &context.coordinator.cancellables)

        
        workspaceInput.focus
            .sink { _ in
                DispatchQueue.main.async {
                    if let currentWrapper = inputManager.currentWrapper, currentWrapper.canBecomeFirstResponder {
                        currentWrapper.becomeFirstResponder()
                    } else {
                        inputManager.mtkView.becomeFirstResponder()
                    }
                }
            }
            .store(in: &context.coordinator.cancellables)
        
        workspaceOutput
            .$currentTab
            .sink { _ in
                DispatchQueue.main.async {
                    inputManager.updateCurrentTab(newCurrentTab: workspaceOutput.currentTab, newTabCount: workspaceOutput.tabCount)
                }
            }
            .store(in: &context.coordinator.cancellables)
        
        return inputManager
    }
    
    public func updateUIView(_ uiView: iOSMTKInputManager, context: Context) {}
}

public class iOSMTKInputManager: UIView, UIGestureRecognizerDelegate {
    public var mtkView: iOSMTK
    
    var currentWrapper: UIView? = nil
    var tabCount: Int = 0
        
    init(_ workspaceInput: WorkspaceInputState, _ workspaceOutput: WorkspaceOutputState, _ coreHandle: UnsafeMutableRawPointer?) {
        mtkView = iOSMTK()
        mtkView.workspaceInput = workspaceInput
        mtkView.workspaceOutput = workspaceOutput
        
        mtkView.setInitialContent(coreHandle)
        
        super.init(frame: .infinite)

        mtkView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(mtkView)
        NSLayoutConstraint.activate([
            mtkView.topAnchor.constraint(equalTo: topAnchor),
            mtkView.leftAnchor.constraint(equalTo: leftAnchor),
            mtkView.rightAnchor.constraint(equalTo: rightAnchor),
            mtkView.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])
    }
            
    required init?(coder aDecoder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    public func updateCurrentTab(newCurrentTab: WorkspaceTab, newTabCount: Int) {
        mtkView.tabSwitchTask = { [weak self] in
            if let inputManager = self {
                inputManager.currentWrapper?.removeFromSuperview()
                
                inputManager.mtkView.onSelectionChanged = nil
                inputManager.mtkView.onTextChanged = nil
                
                inputManager.tabCount = newTabCount
                
                switch newCurrentTab {
                case .Welcome, .Pdf, .Loading, .SpaceInspector:
                    if self?.currentWrapper == nil {
                        return
                    }
                    
                    inputManager.mtkView.currentWrapper = nil
                case .Svg, .Image, .Graph:
                    if self?.currentWrapper is iOSMTKDrawingWrapper {
                        return
                    }
                    
                    let drawingWrapper = iOSMTKDrawingWrapper(mtkView: inputManager.mtkView)
                    inputManager.currentWrapper = drawingWrapper
                    inputManager.mtkView.currentWrapper = drawingWrapper
                                    
                    drawingWrapper.translatesAutoresizingMaskIntoConstraints = false
                    inputManager.addSubview(drawingWrapper)
                    NSLayoutConstraint.activate([
                        drawingWrapper.topAnchor.constraint(equalTo: inputManager.topAnchor, constant: inputManager.mtkView.docHeaderSize),
                        drawingWrapper.leftAnchor.constraint(equalTo: inputManager.leftAnchor),
                        drawingWrapper.rightAnchor.constraint(equalTo: inputManager.rightAnchor),
                        drawingWrapper.bottomAnchor.constraint(equalTo: inputManager.bottomAnchor)
                    ])
                case .PlainText, .Markdown:
                    if self?.currentWrapper is iOSMTKTextInputWrapper {
                        return
                    }
                    
                    let textWrapper = iOSMTKTextInputWrapper(mtkView: inputManager.mtkView)
                    inputManager.currentWrapper = textWrapper
                    inputManager.mtkView.currentWrapper = textWrapper
                    
                    textWrapper.translatesAutoresizingMaskIntoConstraints = false
                    inputManager.addSubview(textWrapper)
                    NSLayoutConstraint.activate([
                        textWrapper.topAnchor.constraint(equalTo: inputManager.topAnchor, constant: inputManager.mtkView.docHeaderSize),
                        textWrapper.leftAnchor.constraint(equalTo: inputManager.leftAnchor),
                        textWrapper.rightAnchor.constraint(equalTo: inputManager.rightAnchor),
                        textWrapper.bottomAnchor.constraint(equalTo: inputManager.bottomAnchor, constant: -iOSMTKTextInputWrapper.TOOL_BAR_HEIGHT)
                    ])
                    
                    if GCKeyboard.coalesced != nil {
                        textWrapper.becomeFirstResponder()
                    }
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
    
    public init(_ workspaceInput: WorkspaceInputState, _ workspaceOutput: WorkspaceOutputState, _ coreHandle: UnsafeMutableRawPointer?) {
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
            .onReceive(workspaceInput.focus, perform: { newValue in
                focused = true
            })
    }
    
    public static func == (lhs: WorkspaceView, rhs: WorkspaceView) -> Bool {
        true
    }
}

public struct NSWS: NSViewRepresentable {
    
    @ObservedObject public var workspaceInput: WorkspaceInputState
    @ObservedObject public var workspaceOutput: WorkspaceOutputState
    
    let coreHandle: UnsafeMutableRawPointer?
        
    public init(_ workspaceInput: WorkspaceInputState, _ workspaceOutput: WorkspaceOutputState, _ coreHandle: UnsafeMutableRawPointer?) {
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
    
    public func makeNSView(context: NSViewRepresentableContext<NSWS>) -> MacMTK {
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
    
    public func updateNSView(_ nsView: MacMTK, context: NSViewRepresentableContext<NSWS>) {}
}
#endif





