import Foundation
import FoundationModels

// Event kinds: 0 = incremental text, 1 = done/available, 2 = error, 3 = tool call.
public typealias LBAppleCallback = @Sendable @convention(c)
    (UInt64, UInt32, UnsafePointer<UInt8>?, Int) -> Void

private func emit(_ id: UInt64, _ callback: LBAppleCallback, _ kind: UInt32, _ text: String = "") {
    Array(text.utf8).withUnsafeBufferPointer { bytes in
        callback(id, kind, bytes.baseAddress, bytes.count)
    }
}

// The lock protects only registration. Never call Rust or cancel a task under it.
private final class Requests: @unchecked Sendable {
    static let shared = Requests()
    private let lock = NSLock()
    private var results: [UInt64: ToolResults] = [:]
    private var tasks: [UInt64: Task<Void, Never>] = [:]

    func start(_ id: UInt64, results: ToolResults, operation: @escaping @Sendable () async -> Void) {
        lock.lock()
        self.results[id] = results
        tasks[id] = Task.detached {
            await operation()
            self.finish(id)
        }
        lock.unlock()
    }

    func finish(_ id: UInt64) {
        lock.lock()
        tasks.removeValue(forKey: id)
        let results = results.removeValue(forKey: id)
        lock.unlock()
        results?.cancel()
    }

    func resolve(_ id: UInt64, call: String, content: String) {
        lock.lock()
        let results = results[id]
        lock.unlock()
        results?.resolve(call, content)
    }

    func cancel(_ id: UInt64) {
        lock.lock()
        let task = tasks.removeValue(forKey: id)
        let results = results.removeValue(forKey: id)
        lock.unlock()
        results?.cancel()
        task?.cancel()
    }
}

// Continuations are removed under the lock and resumed outside it, exactly once.
private final class ToolResults: @unchecked Sendable {
    private let lock = NSLock()
    private var cancelled = false
    private var pending: [String: CheckedContinuation<String, any Error>] = [:]

    func insert(_ id: String, _ continuation: CheckedContinuation<String, any Error>) -> Bool {
        lock.lock()
        let active = !cancelled
        if active { pending[id] = continuation }
        lock.unlock()
        if !active { continuation.resume(throwing: CancellationError()) }
        return active
    }
    func resolve(_ id: String, _ content: String) {
        lock.lock()
        let continuation = pending.removeValue(forKey: id)
        lock.unlock()
        continuation?.resume(returning: content)
    }
    func cancel() {
        lock.lock()
        cancelled = true
        let continuations = Array(pending.values)
        pending.removeAll()
        lock.unlock()
        for continuation in continuations { continuation.resume(throwing: CancellationError()) }
    }
}

@available(macOS 26.0, *)
private struct RustTool: Tool {
    let name: String
    let description: String
    let parameters: GenerationSchema
    let requestID: UInt64
    let callback: LBAppleCallback
    let results: ToolResults

    func call(arguments: GeneratedContent) async throws -> String {
        let callID = UUID().uuidString
        let args = try JSONSerialization.jsonObject(with: Data(arguments.jsonString.utf8))
        let data = try JSONSerialization.data(withJSONObject: ["id": callID, "name": name, "args": args])
        let payload = String(decoding: data, as: UTF8.self)
        return try await withTaskCancellationHandler {
            try Task.checkCancellation()
            return try await withCheckedThrowingContinuation { continuation in
                if results.insert(callID, continuation) { emit(requestID, callback, 3, payload) }
            }
        } onCancel: { results.cancel() }
    }
}

private func unavailableReason() -> String? {
    guard #available(macOS 26.0, *) else {
        return "Apple Intelligence requires macOS 26 or newer."
    }
    switch SystemLanguageModel.default.availability {
    case .available: return nil
    case .unavailable(let reason):
        switch reason {
        case .deviceNotEligible: return "Apple Intelligence requires a supported Apple Silicon Mac."
        case .appleIntelligenceNotEnabled:
            return "Enable Apple Intelligence in System Settings → Apple Intelligence & Siri, then retry."
        case .modelNotReady:
            return "Apple Intelligence is still getting ready. Let its model finish downloading in System Settings, then retry."
        @unknown default: return "Apple Intelligence is unavailable. Check Apple Intelligence & Siri in System Settings."
        }
    }
}

@_cdecl("lb_apple_ai_availability")
public func availability(_ id: UInt64, _ callback: LBAppleCallback) {
    if let reason = unavailableReason() { emit(id, callback, 2, reason) }
    else { emit(id, callback, 1) }
}

private struct Input: Decodable, Sendable {
    struct Message: Decodable, Sendable {
        let role: String
        let content: String
    }
    let instructions: String
    let messages: [Message]
    struct ToolSpec: Decodable, Sendable {
        struct Parameters: Decodable, Sendable {
            struct Property: Decodable, Sendable { let type: String; let description: String? }
            let type: String
            let properties: [String: Property]
            let required: [String]
        }
        let name: String
        let description: String
        let parameters: Parameters
    }
    let tools: [ToolSpec]
    let max_tokens: UInt32
}

@_cdecl("lb_apple_ai_start")
public func start(_ id: UInt64, _ bytes: UnsafePointer<UInt8>?, _ length: Int, _ callback: @escaping LBAppleCallback) {
    guard let bytes, length > 0, length <= 128 * 1024 else {
        emit(id, callback, 2, "Invalid Apple Intelligence request.")
        return
    }
    // Decode/copy before returning: Rust retains ownership of the input buffer.
    let input: Input
    do { input = try JSONDecoder().decode(Input.self, from: Data(bytes: bytes, count: length)) }
    catch { emit(id, callback, 2, "Invalid Apple Intelligence request format."); return }
    if let reason = unavailableReason() { emit(id, callback, 2, reason); return }
    guard #available(macOS 26.0, *) else { return }
    let results = ToolResults()
    Requests.shared.start(id, results: results) { await generate(id, input, callback, results) }
}

@_cdecl("lb_apple_ai_cancel")
public func cancel(_ id: UInt64) { Requests.shared.cancel(id) }

@_cdecl("lb_apple_ai_tool_result")
public func toolResult(_ id: UInt64, _ bytes: UnsafePointer<UInt8>?, _ length: Int) {
    struct Result: Decodable { let id: String; let content: String }
    guard let bytes, length > 0, length <= 128 * 1024,
          let result = try? JSONDecoder().decode(Result.self, from: Data(bytes: bytes, count: length)) else { return }
    Requests.shared.resolve(id, call: result.id, content: result.content)
}

@available(macOS 26.0, *)
private func generate(_ id: UInt64, _ input: Input, _ callback: LBAppleCallback, _ results: ToolResults) async {
    do {
        try Task.checkCancellation()
        guard let last = input.messages.last, last.role == "user" else {
            emit(id, callback, 2, "Apple Intelligence needs a user message to answer.")
            return
        }
        let model = SystemLanguageModel.default
        let outputBudget = min(max(input.max_tokens, 1), 768)
        var entries: [Transcript.Entry] = [
            .instructions(.init(segments: [.text(.init(content: input.instructions))], toolDefinitions: []))
        ]
        for message in input.messages.dropLast() {
            let segments: [Transcript.Segment] = [.text(.init(content: message.content))]
            switch message.role {
            case "user": entries.append(.prompt(.init(segments: segments)))
            case "assistant": entries.append(.response(.init(assetIDs: [], segments: segments)))
            default:
                emit(id, callback, 2, "Unsupported conversation role for Apple Intelligence.")
                return
            }
        }
        let tools: [RustTool] = try input.tools.map { spec in
            guard spec.parameters.type == "object" else { throw BridgeError.unsupportedSchema }
            let properties: [DynamicGenerationSchema.Property] = try spec.parameters.properties.sorted(by: { $0.key < $1.key }).map { key, property in
                let schema: DynamicGenerationSchema
                switch property.type {
                case "string": schema = DynamicGenerationSchema(type: String.self)
                case "boolean": schema = DynamicGenerationSchema(type: Bool.self)
                default: throw BridgeError.unsupportedSchema
                }
                return .init(name: key, description: property.description, schema: schema,
                             isOptional: !spec.parameters.required.contains(key))
            }
            let schema = try GenerationSchema(root: .init(name: spec.name + "Arguments", properties: properties), dependencies: [])
            return RustTool(name: spec.name, description: spec.description, parameters: schema,
                            requestID: id, callback: callback, results: results)
        }
        // Include schemas in the restored instructions as well as registering tools.
        entries[0] = .instructions(.init(segments: [.text(.init(content: input.instructions))],
            toolDefinitions: tools.map { .init(name: $0.name, description: $0.description, parameters: $0.parameters) }))
        let session = LanguageModelSession(model: model, tools: tools, transcript: Transcript(entries: entries))
        #if LB_APPLE_TOKEN_COUNT
        if #available(macOS 26.4, *) {
            let historyTokens = try await model.tokenCount(for: entries)
            let promptTokens = try await model.tokenCount(for: Prompt(last.content))
            guard historyTokens + promptTokens + Int(outputBudget) + 64 <= model.contextSize else {
                emit(id, callback, 2, "This conversation exceeds Apple Intelligence’s \(model.contextSize)-token context. Start a new chat or shorten the note excerpt, then retry.")
                return
            }
        }
        #endif
        try Task.checkCancellation()
        let options = GenerationOptions(temperature: 0.5, maximumResponseTokens: Int(outputBudget))
        var previous = ""
        for try await snapshot in session.streamResponse(to: last.content, options: options) {
            try Task.checkCancellation()
            let content = snapshot.content
            // The shared chat protocol is delta-only. Never concatenate a
            // rewritten snapshot and silently corrupt the user's response.
            guard content.hasPrefix(previous) else {
                emit(id, callback, 2, "Apple Intelligence revised its stream unexpectedly. Please retry.")
                return
            }
            let delta = String(content.dropFirst(previous.count))
            previous = content
            if !delta.isEmpty { emit(id, callback, 0, delta) }
        }
        try Task.checkCancellation()
        emit(id, callback, 1)
    } catch is CancellationError {
        emit(id, callback, 2, "Apple Intelligence request cancelled.")
    } catch {
        // No logging or network reporting. Error text is returned to the chat.
        emit(id, callback, 2, "Apple Intelligence: \(error.localizedDescription) Try a shorter message or start a new chat.")
    }
}

private enum BridgeError: Error { case unsupportedSchema }
