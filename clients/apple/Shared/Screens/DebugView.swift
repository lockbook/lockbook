import SwiftUI
import SwiftWorkspace

struct DebugView: View {
    let debuggg = """
{
  "time": "2025-02-15 23:26:44 -05:00",
  "name": "smail",
  "last_synced": "just now",
  "lb_version": "0.9.18",
  "rust_triple": "aarch64.unix.ios",
  "os_info": "18.3.1",
  "lb_dir": "/var/mobile/Containers/Data/Application/0BA6F188-8B4D-431E-9275-178C3C6507D3/Documents",
  "server_url": "https://api.prod.lockbook.net",
  "integrity": "Ok([EmptyFile(4c05ad9e-3cce-4014-9317-1e2ec1a5e52b), EmptyFile(ce1f4ccc-8be6-4da7-b2f1-66e95717583b), EmptyFile(7a2baf3b-3e02-45e7-8456-5ce48a2a2011), EmptyFile(09be6ecd-5a08-4d79-b606-370b2307c190)])",
  "log_tail": " { temporality: 60, io: 40 }}: lb_rs::service::activity: new\n2025-02-16T04:26:42.729447Z DEBUG suggested_docs{settings=RankingWeights { temporality: 60, io: 40 }}: lb_rs::service::activity: close time.busy=512µs time.idle=62.4µs\n2025-02-16T04:26:42.729563Z DEBUG get_file_by_id{id=36b69132-056e-4296-9988-8769c42447a9}: lb_rs::service::file: new\n2025-02-16T04:26:42.729788Z DEBUG get_file_by_id{id=36b69132-056e-4296-9988-8769c42447a9}: lb_rs::service::file: close time.busy=203µs time.idle=21.8µs\n2025-02-16T04:26:42.729822Z DEBUG get_file_by_id{id=60c826a1-fab7-4984-a1a8-307323d0480f}: lb_rs::service::file: new\n2025-02-16T04:26:42.730023Z DEBUG get_file_by_id{id=60c826a1-fab7-4984-a1a8-307323d0480f}: lb_rs::service::file: close time.busy=188µs time.idle=13.6µs\n2025-02-16T04:26:42.730044Z DEBUG get_file_by_id{id=5f2d65a7-a855-40c3-9a38-64ba87ec3a4c}: lb_rs::service::file: new\n2025-02-16T04:26:42.730240Z DEBUG get_file_by_id{id=5f2d65a7-a855-40c3-9a38-64ba87ec3a4c}: lb_rs::service::file: close time.busy=185µs time.idle=11.4µs\n2025-02-16T04:26:42.730260Z DEBUG get_file_by_id{id=528ebc9c-3daa-4f3d-952a-3978e8b1f58d}: lb_rs::service::file: new\n2025-02-16T04:26:42.730452Z DEBUG get_file_by_id{id=528ebc9c-3daa-4f3d-952a-3978e8b1f58d}: lb_rs::service::file: close time.busy=182µs time.idle=10.5µs\n2025-02-16T04:26:42.730469Z DEBUG get_file_by_id{id=3c09d308-5e89-4d44-a290-ef1482a41e56}: lb_rs::service::file: new\n2025-02-16T04:26:42.730719Z DEBUG get_file_by_id{id=3c09d308-5e89-4d44-a290-ef1482a41e56}: lb_rs::service::file: close time.busy=240µs time.idle=10.5µs\n2025-02-16T04:26:42.730761Z DEBUG get_file_by_id{id=07c8fad8-b610-44da-81a3-fc747cb0d2f0}: lb_rs::service::file: new\n2025-02-16T04:26:42.730975Z DEBUG get_file_by_id{id=07c8fad8-b610-44da-81a3-fc747cb0d2f0}: lb_rs::service::file: close time.busy=198µs time.idle=15.6µs\n2025-02-16T04:26:42.730998Z DEBUG get_file_by_id{id=4b8be938-2f7d-414c-bc23-209de9856743}: lb_rs::service::file: new\n2025-02-16T04:26:42.731202Z DEBUG get_file_by_id{id=4b8be938-2f7d-414c-bc23-209de9856743}: lb_rs::service::file: close time.busy=193µs time.idle=11.6µs\n2025-02-16T04:26:42.731223Z DEBUG get_file_by_id{id=e5eacd76-62c7-4f79-b9f4-d3ea61f88c11}: lb_rs::service::file: new\n2025-02-16T04:26:42.731415Z DEBUG get_file_by_id{id=e5eacd76-62c7-4f79-b9f4-d3ea61f88c11}: lb_rs::service::file: close time.busy=182µs time.idle=10.0µs\n2025-02-16T04:26:42.778593Z DEBUG background_file_cache_refresh{thread=\"ThreadId(54)\"}:get_usage:request{route=\"/get-usage\"}: lb_rs::service::network: close time.busy=1.33ms time.idle=61.8ms\n2025-02-16T04:26:42.778651Z DEBUG background_file_cache_refresh{thread=\"ThreadId(54)\"}:get_usage: lb_rs::service::usage: close time.busy=1.42ms time.idle=61.9ms\n2025-02-16T04:26:42.778669Z DEBUG background_file_cache_refresh{thread=\"ThreadId(54)\"}: workspace_rs::task_manager: close time.busy=89.0ms time.idle=232µs\n2025-02-16T04:26:43.716034Z DEBUG background_sync_status_update{thread=\"ThreadId(55)\"}: workspace_rs::task_manager: new\n2025-02-16T04:26:43.716205Z DEBUG background_sync_status_update{thread=\"ThreadId(55)\"}:get_pending_shares: lb_rs::service::share: new\n2025-02-16T04:26:43.719191Z DEBUG background_sync_status_update{thread=\"ThreadId(55)\"}:get_pending_shares: lb_rs::service::share: close time.busy=2.94ms time.idle=47.5µs\n2025-02-16T04:26:43.719277Z DEBUG background_sync_status_update{thread=\"ThreadId(55)\"}: workspace_rs::task_manager: close time.busy=3.10ms time.idle=141µs\n2025-02-16T04:26:43.861674Z DEBUG debug_info{os_info=\"18.3.1\"}: lb_rs::service::debug: new\n2025-02-16T04:26:43.865677Z DEBUG debug_info{os_info=\"18.3.1\"}:test_repo_integrity: lb_rs::service::integrity: new\n2025-02-16T04:26:43.958033Z DEBUG debug_info{os_info=\"18.3.1\"}:test_repo_integrity:list_metadatas: lb_rs::service::file: new\n2025-02-16T04:26:43.983666Z DEBUG debug_info{os_info=\"18.3.1\"}:test_repo_integrity:list_metadatas: lb_rs::service::file: close time.busy=25.5ms time.idle=130µs\n2025-02-16T04:26:43.984684Z DEBUG debug_info{os_info=\"18.3.1\"}:test_repo_integrity:read_document{id=c6b5ea29-1bc7-452d-b4ea-0f9317433e80 user_activity=false}: lb_rs::service::documents: new\n2025-02-16T04:26:43.984889Z DEBUG debug_info{os_info=\"18.3.1\"}:test_repo_integrity:read_document{id=ce1f4ccc-8be6-4da7-b2f1-66e95717583b user_activity=false}: lb_rs::service::documents: new\n2025-02-16T04:26:43.985050Z DEBUG debug_info{os_info=\"18.3.1\"}:test_repo_integrity:read_document{id=60c826a1-fab7-4984-a1a8-307323d0480f user_activity=false}: lb_rs::service::documents: new\n2025-02-16T04:26:43.985201Z DEBUG debug_info{os_info=\"18.3.1\"}:test_repo_integrity:read_document{id=4c05ad9e-3cce-4014-9317-1e2ec1a5e52b user_activity=false}: lb_rs::service::documents: new\n2025-02-16T04:26:43.985444Z DEBUG debug_info{os_info=\"18.3.1\"}:test_repo_integrity:read_document{id=c8b48516-e63d-4260-b39a-fa7b15265ebd user_activity=false}: lb_rs::service::documents: new\n2025-02-16T04:26:43.985540Z DEBUG debug_info{os_info=\"18.3.1\"}:test_repo_integrity:read_document{id=c8998273-8a0d-42a8-afce-ff4d0d7bd05d user_activity=false}: lb_rs::service::documents: new\n",
  "last_panic": ""
}
"""
    
    @State var debugInfo: String? = nil
    @State var copied = false
    
    var body: some View {
        VStack {
            if let debugInfo {
                ScrollView {
                    Spacer()
                    
                    Text(debuggg)
                        .monospaced()
                        .padding()
                    
                    Spacer()
                }
                .toolbar {
                    Button(action: {
                        ClipboardHelper.copyToClipboard(debugInfo)
                    }, label: {
                        Image(systemName: "doc.on.doc")
                    })
                }
            } else {
                Spacer()
                
                ProgressView()
                    .onAppear {
                        DispatchQueue.global(qos: .userInitiated).async {
                            let debug = AppState.lb.debugInfo()
                            DispatchQueue.main.async {
                                debugInfo = debug
                            }
                        }
                    }
                
                Spacer()
            }
        }
        .navigationTitle("Debug Info")
    }
}

#Preview {
    NavigationStack {
        DebugView()
    }
}
