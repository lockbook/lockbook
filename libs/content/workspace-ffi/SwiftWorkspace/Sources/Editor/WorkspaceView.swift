import Foundation
import SwiftUI
import MetalKit
import Combine
import Bridge

#if os(iOS)
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
    
    public static var mtkView: iOSMTKInputManager? = nil
    
    public init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        self.workspaceState = workspaceState
        self.coreHandle = coreHandle
    }

    public func makeUIView(context: Context) -> iOSMTKInputManager {
        if Self.mtkView == nil {
            Self.mtkView = iOSMTKInputManager(workspaceState, coreHandle)
        }
        
        return Self.mtkView!
    }
    
    public func updateUIView(_ uiView: iOSMTKInputManager, context: Context) {
        uiView.mtkView.showTabs = horizontal == .regular && vertical == .regular
        
        if let id = workspaceState.openDoc {
            if uiView.mtkView.currentOpenDoc != id {
                uiView.mtkView.openFile(id: id)
            }
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
        
        if workspaceState.renameCompleted != nil {
            uiView.mtkView.openDocRenamed(newName: workspaceState.renameCompleted!)
            workspaceState.renameCompleted = nil
        }
    }
}

public class iOSMTKInputManager: UIView {
    public var mtkView: iOSMTK
    
    var currentWrapper: UIView? = nil
    var currentTab: WorkspaceTab = .Welcome
    
    init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        mtkView = iOSMTK()
        mtkView.workspaceState = workspaceState
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
    
    public func updateCurrentTab(newCurrentTab: WorkspaceTab) {
        mtkView.tabSwitchTask = {
            self.currentWrapper?.removeFromSuperview()
            
            self.mtkView.onSelectionChanged = nil
            self.mtkView.onTextChanged = nil
            
            self.currentTab = newCurrentTab
            
            switch self.currentTab {
            case .Welcome, .Pdf, .Loading, .Image:
                print("wrapper not needed")
            case .Svg:
                let drawingWrapper = iOSMTKDrawingWrapper(mtkView: self.mtkView)
                self.currentWrapper = drawingWrapper
                                
                drawingWrapper.translatesAutoresizingMaskIntoConstraints = false
                self.addSubview(drawingWrapper)
                NSLayoutConstraint.activate([
                    drawingWrapper.topAnchor.constraint(equalTo: self.topAnchor, constant: iOSMTK.TAB_BAR_HEIGHT + iOSMTKDrawingWrapper.TOOL_BAR_HEIGHT),
                    drawingWrapper.leftAnchor.constraint(equalTo: self.leftAnchor),
                    drawingWrapper.rightAnchor.constraint(equalTo: self.rightAnchor),
                    drawingWrapper.bottomAnchor.constraint(equalTo: self.bottomAnchor)
                ])
            case .PlainText, .Markdown:
                let textWrapper = iOSMTKTextInputWrapper(mtkView: self.mtkView)
                self.currentWrapper = textWrapper
                
                textWrapper.translatesAutoresizingMaskIntoConstraints = false
                self.addSubview(textWrapper)
                NSLayoutConstraint.activate([
                    textWrapper.topAnchor.constraint(equalTo: self.topAnchor, constant: iOSMTK.TAB_BAR_HEIGHT),
                    textWrapper.leftAnchor.constraint(equalTo: self.leftAnchor),
                    textWrapper.rightAnchor.constraint(equalTo: self.rightAnchor),
                    textWrapper.bottomAnchor.constraint(equalTo: self.bottomAnchor, constant: -iOSMTKTextInputWrapper.TOOL_BAR_HEIGHT)
                ])
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
    let mtkView: MacMTK = MacMTK()
    
    public init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        self.workspaceState = workspaceState
        mtkView.workspaceState = workspaceState
        self.coreHandle = coreHandle
        
    }
    
    public func makeNSView(context: NSViewRepresentableContext<NSWS>) -> MTKView {
        mtkView.setInitialContent(coreHandle)
        return mtkView
    }
    
    public func updateNSView(_ nsView: MTKView, context: NSViewRepresentableContext<NSWS>) {
        if let id = workspaceState.openDoc {
            if mtkView.currentOpenDoc != id {
                mtkView.openFile(id: id)
            }
        }
        
        if workspaceState.shouldFocus {
            // todo?
            workspaceState.shouldFocus = false
        }
        
        if workspaceState.syncRequested {
            workspaceState.syncRequested = false
            mtkView.requestSync()
        }
    }
}
#endif





