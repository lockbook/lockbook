import Foundation
import SwiftUI
import MetalKit
import Combine

#if os(iOS)
public struct WorkspaceView: View, Equatable {
    
    let workspaceState: WorkspaceState
    let coreHandle: UnsafeMutableRawPointer?
    
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
    
    public init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        self.workspaceState = workspaceState
        self.coreHandle = coreHandle
    }

    public func makeUIView(context: Context) -> iOSMTKInputManager {
        let mtkView = iOSMTKInputManager(workspaceState, coreHandle)

        return mtkView
    }
    
    public func updateUIView(_ uiView: iOSMTKInputManager, context: Context) {
        if let id = workspaceState.openDoc {
            if uiView.mtkView.currentOpenDoc != id {
                uiView.mtkView.openFile(id: id)
            }
        }
        
        if workspaceState.shouldFocus {
            workspaceState.shouldFocus = false
            uiView.currentWrapper.becomeFirstResponder()
        }
        
        if workspaceState.syncRequested {
            workspaceState.syncRequested = false
            uiView.mtkView.requestSync()
        }
        
        if workspaceState.currentTab.viewWrapperIdentifier() != uiView.currentTab.viewWrapperIdentifier() {
            uiView.updateCurrentTab(newCurrentTab: workspaceState.currentTab)
        }
    }
}

public class iOSMTKInputManager: UIView {
    public var mtkView: iOSMTK
    
    var currentWrapper: UIView = UIView()
    var currentTab: WorkspaceTab = .Welcome
    
    init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        mtkView = iOSMTK()
        mtkView.workspaceState = workspaceState
        mtkView.setInitialContent(coreHandle)
        
        currentWrapper.addSubview(mtkView)
        
        super.init(frame: .infinite)
        
        addSubview(currentWrapper)

        mtkView.translatesAutoresizingMaskIntoConstraints = false
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
        print("scheduled switch")
        
        mtkView.tabSwitchTask = {
            self.mtkView.removeConstraints(self.mtkView.constraints)
            
            self.mtkView.removeFromSuperview()
            self.currentWrapper.removeFromSuperview()
            self.mtkView.onSelectionChanged = nil
            self.mtkView.onTextChanged = nil
            
            self.currentTab = newCurrentTab
            
            switch self.currentTab {
            case .Welcome, .Pdf, .Loading, .Image:
                self.currentWrapper = UIView()
                self.currentWrapper.addSubview(self.mtkView)
            case .Svg:
                self.currentWrapper = iOSMTKDrawingWrapper(mtkView: self.mtkView)
            case .PlainText, .Markdown:
                self.currentWrapper = iOSMTKTextInputWrapper(mtkView: self.mtkView)
            }
            
            self.addSubview(self.currentWrapper)

            self.currentWrapper.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                self.currentWrapper.topAnchor.constraint(equalTo: self.topAnchor),
                self.currentWrapper.leftAnchor.constraint(equalTo: self.leftAnchor),
                self.currentWrapper.rightAnchor.constraint(equalTo: self.rightAnchor),
                self.currentWrapper.bottomAnchor.constraint(equalTo: self.bottomAnchor)
            ])
                        
            self.mtkView.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                self.mtkView.topAnchor.constraint(equalTo: self.topAnchor),
                self.mtkView.leftAnchor.constraint(equalTo: self.leftAnchor),
                self.mtkView.rightAnchor.constraint(equalTo: self.rightAnchor),
                self.mtkView.bottomAnchor.constraint(equalTo: self.bottomAnchor)
            ])
            
            self.currentWrapper.becomeFirstResponder()
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





