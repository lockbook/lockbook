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
        
    var openDoc: UUID? = nil
    
    static var inputManager: iOSMTKInputManager? = nil
        
    public init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        self.workspaceState = workspaceState
        self.coreHandle = coreHandle
    }

    public func makeUIView(context: Context) -> iOSMTKInputManager {
        if Self.inputManager == nil {
            Self.inputManager = iOSMTKInputManager(workspaceState, coreHandle)
        }
        
        return Self.inputManager!
    }
    
    public func updateUIView(_ uiView: iOSMTKInputManager, context: Context) {
        if uiView.mtkView.showTabs != workspaceState.showTabs {
            uiView.mtkView.showHideTabs(show: workspaceState.showTabs)
        }
        
        if let id = workspaceState.openDocRequested {
            uiView.mtkView.openFile(id: id)
            DispatchQueue.main.async {
                workspaceState.openDocRequested = nil
            }
        }
        
        if workspaceState.closeAllTabsRequested {
            DispatchQueue.main.async {
                workspaceState.closeAllTabsRequested = false
            }
            uiView.mtkView.closeAllTabs()
        }
        
        if workspaceState.currentTab.viewWrapperId() != uiView.currentTab.viewWrapperId() || workspaceState.tabCount != uiView.tabCount {
            uiView.updateCurrentTab(newCurrentTab: workspaceState.currentTab, newTabCount: workspaceState.tabCount)
        }
        
        if workspaceState.shouldFocus {
            DispatchQueue.main.async {
                workspaceState.shouldFocus = false
            }
            uiView.currentWrapper?.becomeFirstResponder()
        }
        
        if workspaceState.syncRequested {
            DispatchQueue.main.async {
                workspaceState.syncRequested = false
            }
            uiView.mtkView.requestSync()
        }
        
        if workspaceState.fileOpCompleted != nil {
            uiView.mtkView.fileOpCompleted(fileOp: workspaceState.fileOpCompleted!)
            DispatchQueue.main.async {
                workspaceState.fileOpCompleted = nil
            }
        }
        
        if let id = workspaceState.closeDocRequested {
            DispatchQueue.main.async {
                workspaceState.closeDocRequested = nil
            }
            let activeDoc = workspaceState.openDoc
            uiView.mtkView.closeDoc(id: id)
            if activeDoc == id {
                uiView.currentWrapper?.resignFirstResponder()
            }
        }
    }
}

public class iOSMTKInputManager: UIView, UIGestureRecognizerDelegate {
    public var mtkView: iOSMTK
    
    var currentWrapper: UIView? = nil
    var currentTab: WorkspaceTab = .Welcome
    var tabCount: Int = 0
        
    init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        mtkView = iOSMTK()
        mtkView.workspaceState = workspaceState
        
        print("showtabs: ", workspaceState.showTabs)
        mtkView.setInitialContent(coreHandle, showTabs: workspaceState.showTabs)
        
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
                
                inputManager.currentTab = newCurrentTab
                inputManager.tabCount = newTabCount
                
                switch inputManager.currentTab {
                case .Welcome, .Pdf, .Loading, .SpaceInspector:
                    inputManager.mtkView.currentWrapper = nil
                case .Svg, .Image, .Graph:
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
        
        if let id = workspaceState.closeDocRequested {
            nsView.closeDoc(id: id)
        }
    }
}
#endif





