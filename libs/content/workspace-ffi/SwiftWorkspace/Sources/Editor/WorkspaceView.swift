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

    public func makeUIView(context: Context) -> iOSMTK {
        let mtkView = iOSMTK()
        mtkView.workspaceState = workspaceState
        mtkView.setInitialContent(coreHandle)

        return mtkView
    }
    
    public func updateUIView(_ uiView: iOSMTK, context: Context) {
        if let id = workspaceState.openDoc {
            if uiView.currentOpenDoc != id {
                uiView.openFile(id: id)
            }
        }
        
        if workspaceState.shouldFocus {
            uiView.becomeFirstResponder()
            workspaceState.shouldFocus = false
        }
        
        if workspaceState.syncRequested {
            workspaceState.syncRequested = false
            uiView.requestSync()
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





