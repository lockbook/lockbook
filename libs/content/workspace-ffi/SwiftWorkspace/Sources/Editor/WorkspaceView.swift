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

    public func makeUIView(context: Context) -> iOSMTKTouchWrapper {
        let mtkView = iOSMTKTouchWrapper(workspaceState, coreHandle)
        
        

        return mtkView
    }
    
    public func updateUIView(_ uiView: iOSMTKTouchWrapper, context: Context) {
        if let id = workspaceState.openDoc {
            if uiView.mtkView.currentOpenDoc != id {
                uiView.mtkView.openFile(id: id)
            }
        }
        
        if workspaceState.shouldFocus {
            uiView.mtkView.becomeFirstResponder()
            workspaceState.shouldFocus = false
        }
        
        if workspaceState.syncRequested {
            workspaceState.syncRequested = false
            uiView.mtkView.requestSync()
        }
    }
}

public class iOSMTKTouchWrapper: UIView {
    public var mtkView: iOSMTK
    
    init(_ workspaceState: WorkspaceState, _ coreHandle: UnsafeMutableRawPointer?) {
        mtkView = iOSMTK()
        mtkView.workspaceState = workspaceState
        mtkView.setInitialContent(coreHandle)
        
        super.init(frame: .infinite)
        
        addSubview(mtkView)

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
    
    public override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        mtkView.forwardedTouchesBegan(touches, with: event)
        
        if touches.first?.type == UITouch.TouchType.pencil && mtkView.currentTab == .Svg {
            print("not forwarding touches in began")
            return
        }
        
        super.touchesBegan(touches, with: event)
    }
    
    public override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        mtkView.forwardedTouchesMoved(touches, with: event)
        
        if touches.first?.type == UITouch.TouchType.pencil && mtkView.currentTab == .Svg {
            print("not forwarding touches in moved")
            return
        }
        
        super.touchesMoved(touches, with: event)
    }
    
    public override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        mtkView.forwardedTouchesEnded(touches, with: event)
        
        if touches.first?.type == UITouch.TouchType.pencil && mtkView.currentTab == .Svg {
            print("not forwarding touches in ended")
            return
        }
        
        super.touchesEnded(touches, with: event)
    }
    
    public override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        mtkView.forwardedTouchesCancelled(touches, with: event)
        
        if touches.first?.type == UITouch.TouchType.pencil && mtkView.currentTab == .Svg {
            print("not forwarding touches in canceled")
            return
        }
        
        super.touchesCancelled(touches, with: event)
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





