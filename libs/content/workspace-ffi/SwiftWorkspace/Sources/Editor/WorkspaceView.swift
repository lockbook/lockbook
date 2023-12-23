import Foundation
import SwiftUI
import MetalKit
import Combine

#if os(iOS)
public struct WorkspaceView: UIViewRepresentable {
    
    @ObservedObject public var workspaceState: WorkspaceState
    let mtkView: iOSMTK = iOSMTK()
    
    @Environment(\.horizontalSizeClass) var horizontal
    @Environment(\.verticalSizeClass) var vertical
    
    public init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?, _ toolbarState: ToolbarState, _ nameState: NameState) {
        self.workspaceState = workspaceState
        mtkView.workspaceState = workspaceState
        mtkView.toolbarState = toolbarState
        mtkView.nameState = nameState
        
        mtkView.setInitialContent(coreHandle, workspaceState.text)
    }

    public func makeUIView(context: Context) -> iOSMTK {
        return mtkView
    }
    
    public func updateUIView(_ uiView: iOSMTK, context: Context) {
        if workspaceState.reloadText {
            mtkView.updateText(workspaceState.text)
            workspaceState.reloadText = false
        }
        
        if workspaceState.reloadView {
            mtkView.setNeedsDisplay(mtkView.frame)
            workspaceState.reloadView = false
        }
        
        if workspaceState.shouldFocus {
            mtkView.becomeFirstResponder()
            workspaceState.shouldFocus = false
        }
    }
}
#else
public struct WorkspaceView: View, Equatable {
    @FocusState var focused: Bool
    @ObservedObject var workspaceState: WorkspaceState
    
    let nsEditorView: NSWorkspace
    
    public init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        self.workspaceState = workspaceState
        nsEditorView = NSWorkspace(workspaceState, coreHandle)
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

public struct NSWorkspace: NSViewRepresentable {
    
    @ObservedObject public var workspaceState: WorkspaceState
    let coreHandle: UnsafeMutableRawPointer?
    let mtkView: MacMTK = MacMTK()
    
    public init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        self.workspaceState = workspaceState
        mtkView.workspaceState = workspaceState
        self.coreHandle = coreHandle
        
    }
    
    public func makeNSView(context: NSViewRepresentableContext<NSWorkspace>) -> MTKView {
        mtkView.setInitialContent(coreHandle)
        return mtkView
    }
    
    public func updateNSView(_ nsView: MTKView, context: NSViewRepresentableContext<NSWorkspace>) {
        if workspaceState.reloadView { // todo who is asking for this?
            mtkView.setNeedsDisplay(mtkView.frame)
            workspaceState.reloadView = false
        }
        
        if let id = workspaceState.openDoc {
            print(id)
            mtkView.openFile(id: id)
        }
        
        if workspaceState.shouldFocus {
            workspaceState.shouldFocus = false
        }
    }
}
#endif





