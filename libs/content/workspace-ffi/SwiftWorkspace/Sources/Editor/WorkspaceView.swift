import Foundation
import SwiftUI
import MetalKit
import Combine
import Bridge

#if os(iOS)
import GameController

public struct WorkspaceView: View, Equatable {
    
    let workspaceState: WorkspaceState
    let coreHandle: UnsafeMutableRawPointer?
    
    @State var activeTabName = ""
    
    public init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        self.workspaceState = workspaceState
        self.coreHandle = coreHandle
    }
    
    public var body: some View {
        UIWS(workspaceState, coreHandle)
    }
    
    public static func == (lhs: WorkspaceView, rhs: WorkspaceView) -> Bool {
        return true
    }
}

public struct UIWS: UIViewRepresentable {
    
    @ObservedObject public var workspaceState: WorkspaceState
    let coreHandle: UnsafeMutableRawPointer?
    
    @Environment(\.horizontalSizeClass) var horizontal
    @Environment(\.verticalSizeClass) var vertical
    
    var openDoc: UUID? = nil
        
    public init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        self.workspaceState = workspaceState
        self.coreHandle = coreHandle
    }

    public func makeUIView(context: Context) -> iOSMTKInputManager {
        return iOSMTKInputManager(workspaceState, coreHandle)
    }
    
    public func updateUIView(_ uiView: iOSMTKInputManager, context: Context) {
        let showTabs = horizontal == .regular && vertical == .regular
        if uiView.mtkView.showTabs != showTabs {
            uiView.mtkView.showHideTabs(show: showTabs)
        }
        
        if let id = workspaceState.openDocRequested {
            uiView.mtkView.openFile(id: id)
            workspaceState.openDocRequested = nil
        }
        
        if workspaceState.currentTab.viewWrapperId() != uiView.currentTab.viewWrapperId() {
            uiView.updateCurrentTab(newCurrentTab: workspaceState.currentTab)
        }
        
        if workspaceState.shouldFocus {
            workspaceState.shouldFocus = false
            uiView.currentWrapper?.becomeFirstResponder()
        }
        
        if workspaceState.syncRequested {
            workspaceState.syncRequested = false
            uiView.mtkView.requestSync()
        }
        
        if workspaceState.fileOpCompleted != nil {
            uiView.mtkView.fileOpCompleted(fileOp: workspaceState.fileOpCompleted!)
            workspaceState.fileOpCompleted = nil
        }
        
        if workspaceState.closeActiveTab {
            workspaceState.closeActiveTab = false
            uiView.mtkView.closeActiveTab()
            uiView.currentWrapper?.resignFirstResponder()
        }
    }
}

public class iOSMTKInputManager: UIView, UIGestureRecognizerDelegate {
    public var mtkView: iOSMTK
    
    var currentWrapper: UIView? = nil
    var currentTab: WorkspaceTab = .Welcome
        
    init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        mtkView = iOSMTK()
        mtkView.workspaceState = workspaceState
        mtkView.setInitialContent(coreHandle)
        
        super.init(frame: .infinite)
        
        let pan = UIPanGestureRecognizer(target: self, action: #selector(self.onPan(_:)))
        pan.delegate = self
        addGestureRecognizer(pan)
                
        mtkView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(mtkView)
        NSLayoutConstraint.activate([
            mtkView.topAnchor.constraint(equalTo: topAnchor),
            mtkView.leftAnchor.constraint(equalTo: leftAnchor),
            mtkView.rightAnchor.constraint(equalTo: rightAnchor),
            mtkView.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])
    }
    
    
    public func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        return gestureRecognizer is UIPanGestureRecognizer && touch.location(in: self).x < 40 && !mtkView.showTabs
    }
    
    @objc func onPan(_ sender: UIPanGestureRecognizer? = nil) {
        if mtkView.showTabs {
            return
        }
        
        guard let sender = sender else {
            return
        }
                
        switch sender.state {
        case .ended:
            if sender.translation(in: self).x > 100 || sender.velocity(in: self).x > 200 {
                withAnimation {
                    mtkView.workspaceState?.closeActiveTab = true
                    mtkView.workspaceState!.dragOffset = 0
                }
            } else {
                withAnimation {
                    mtkView.workspaceState!.dragOffset = 0
                }
            }
        case .changed:
            let translation = sender.translation(in: self).x
            
            if translation > 0 {
                withAnimation {
                    mtkView.workspaceState!.dragOffset = sender.translation(in: self).x
                }
            }
        default:
            print("unrecognized drag state")
        }
    }
    
    required init?(coder aDecoder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    public func updateCurrentTab(newCurrentTab: WorkspaceTab) {
        mtkView.tabSwitchTask = { [weak self] in
            if let inputManager = self {
                inputManager.currentWrapper?.removeFromSuperview()
                
                inputManager.mtkView.onSelectionChanged = nil
                inputManager.mtkView.onTextChanged = nil
                
                inputManager.currentTab = newCurrentTab
                
                switch inputManager.currentTab {
                case .Welcome, .Pdf, .Loading, .Image:
                    print("wrapper not needed")
                case .Svg:
                    let drawingWrapper = iOSMTKDrawingWrapper(mtkView: inputManager.mtkView)
                    inputManager.currentWrapper = drawingWrapper
                                    
                    drawingWrapper.translatesAutoresizingMaskIntoConstraints = false
                    inputManager.addSubview(drawingWrapper)
                    NSLayoutConstraint.activate([
                        drawingWrapper.topAnchor.constraint(equalTo: inputManager.topAnchor, constant: iOSMTK.TAB_BAR_HEIGHT + iOSMTKDrawingWrapper.TOOL_BAR_HEIGHT),
                        drawingWrapper.leftAnchor.constraint(equalTo: inputManager.leftAnchor),
                        drawingWrapper.rightAnchor.constraint(equalTo: inputManager.rightAnchor),
                        drawingWrapper.bottomAnchor.constraint(equalTo: inputManager.bottomAnchor)
                    ])
                case .PlainText, .Markdown:
                    let textWrapper = iOSMTKTextInputWrapper(mtkView: inputManager.mtkView)
                    inputManager.currentWrapper = textWrapper
                    
                    textWrapper.translatesAutoresizingMaskIntoConstraints = false
                    inputManager.addSubview(textWrapper)
                    NSLayoutConstraint.activate([
                        textWrapper.topAnchor.constraint(equalTo: inputManager.topAnchor, constant: iOSMTK.TAB_BAR_HEIGHT),
                        textWrapper.leftAnchor.constraint(equalTo: inputManager.leftAnchor),
                        textWrapper.rightAnchor.constraint(equalTo: inputManager.rightAnchor),
                        textWrapper.bottomAnchor.constraint(equalTo: inputManager.bottomAnchor, constant: -iOSMTKTextInputWrapper.TOOL_BAR_HEIGHT)
                    ])
//                    
//                    if GCKeyboard.coalesced != nil {
//                        textWrapper.becomeFirstResponder()
//                    }
                }
            }
        }
    }
}

#else
public struct WorkspaceView: View, Equatable {
    @FocusState var focused: Bool
    @ObservedObject var workspaceState: WorkspaceState
    
    let nsEditorView: NSWS
    
    public init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        self.workspaceState = workspaceState
        nsEditorView = NSWS(workspaceState, coreHandle)
    }
    
    public var body: some View {
        nsEditorView
            .focused($focused)
            .onAppear {
                focused = true
            }
            .onChange(of: workspaceState.shouldFocus, perform: { newValue in
                if newValue {
                    focused = true
                }
            })

    }
    
    public static func == (lhs: WorkspaceView, rhs: WorkspaceView) -> Bool {
        true
    }
}

public struct NSWS: NSViewRepresentable {
    
    @ObservedObject public var workspaceState: WorkspaceState
    let coreHandle: UnsafeMutableRawPointer?
    
    public init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        self.workspaceState = workspaceState
        self.coreHandle = coreHandle
    }
    
    public func makeNSView(context: NSViewRepresentableContext<NSWS>) -> MacMTK {
        let mtkView = MacMTK()
        mtkView.workspaceState = workspaceState
        mtkView.setInitialContent(coreHandle)
        
        return mtkView
    }
    
    public func updateNSView(_ nsView: MacMTK, context: NSViewRepresentableContext<NSWS>) {
        if let id = workspaceState.openDocRequested {
            nsView.openFile(id: id)
            workspaceState.openDocRequested = nil
        }
        
        if workspaceState.shouldFocus {
            // todo?
            workspaceState.shouldFocus = false
        }
        
        if workspaceState.syncRequested {
            workspaceState.syncRequested = false
            nsView.requestSync()
        }
        
        if workspaceState.fileOpCompleted != nil {
            nsView.fileOpCompleted(fileOp: workspaceState.fileOpCompleted!)
            workspaceState.fileOpCompleted = nil
        }
    }
}
#endif





