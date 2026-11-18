#if os(iOS)
    import AVFoundation
    import Bridge
    import Foundation
    import Speech

    @MainActor
    final class VoiceHost {
        static let shared = VoiceHost()

        private var outbox: [String] = []

        private var recorder: AVAudioRecorder?
        private var recordingID: String?
        private var startingID: String?
        private var stopWhenStarted: Set<String> = []

        private var player: AVAudioPlayer?
        private var playerID: String?
        private var playerPath: String?
        private var reportedPlaying = false

        private var transcriptions: [String: Task<Void, Never>] = [:]
        private var transcriptionFraction: [String: Double] = [:]

        private struct Request: Decodable {
            let op: String
            let id: String
            let path: String?
            let from_ms: UInt64?
            let ms: UInt64?
        }

        func pump(_ ws: UnsafeMutableRawPointer?) -> Bool {
            if let raw = voice_requests(ws) {
                let json = String(cString: raw)
                free_text(raw)
                if let data = json.data(using: .utf8),
                   let requests = try? JSONDecoder().decode([Request].self, from: data)
                {
                    for request in requests {
                        handle(request)
                    }
                }
            }

            if let recorder, let id = recordingID, recorder.isRecording {
                recorder.updateMeters()
                let db = recorder.averagePower(forChannel: 0)
                let level = max(0, min(1, (db + 50) / 50))
                post([
                    "event": "recording", "id": id,
                    "elapsed_ms": Int(recorder.currentTime * 1000), "level": level,
                ])
            }

            if let player, let id = playerID {
                let playing = player.isPlaying
                if playing || playing != reportedPlaying {
                    post([
                        "event": "playback", "id": id,
                        "position_ms": Int(player.currentTime * 1000), "playing": playing,
                    ])
                    reportedPlaying = playing
                }
            }

            let delivered = !outbox.isEmpty
            for event in outbox {
                event.withCString { voice_event(ws, $0) }
            }
            outbox.removeAll()
            return delivered
        }

        private func post(_ event: [String: Any]) {
            guard let data = try? JSONSerialization.data(withJSONObject: event),
                  let json = String(data: data, encoding: .utf8)
            else { return }
            outbox.append(json)
        }

        private func handle(_ request: Request) {
            switch request.op {
            case "record":
                record(id: request.id)
            case "stop":
                stopRecording(id: request.id)
            case "play":
                if let path = request.path {
                    play(id: request.id, path: path, fromMs: request.from_ms ?? 0)
                }
            case "pause":
                if playerID == request.id { player?.pause() }
            case "seek":
                if playerID == request.id, let ms = request.ms {
                    player?.currentTime = Double(ms) / 1000
                }
            case "transcribe":
                if let path = request.path {
                    transcribe(id: request.id, path: path)
                }
            case "release":
                release(id: request.id)
            default:
                break
            }
        }

        nonisolated private static func activateSession(recording: Bool) throws {
            let session = AVAudioSession.sharedInstance()
            if recording {
                try session.setCategory(
                    .playAndRecord, mode: .default, options: [.defaultToSpeaker, .allowBluetoothHFP]
                )
            } else {
                try session.setCategory(.playback, mode: .spokenAudio)
            }
            try session.setActive(true)
        }

        nonisolated private static func startRecorder() throws -> AVAudioRecorder {
            try activateSession(recording: true)
            let url = FileManager.default.temporaryDirectory
                .appendingPathComponent("voice-recording-\(UUID().uuidString).m4a")
            let settings: [String: Any] = [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 44100,
                AVNumberOfChannelsKey: 1,
                AVEncoderBitRateKey: 64000,
                AVEncoderAudioQualityKey: AVAudioQuality.high.rawValue,
            ]
            let recorder = try AVAudioRecorder(url: url, settings: settings)
            recorder.isMeteringEnabled = true
            guard recorder.record() else {
                throw VoiceError("The recording couldn't start.")
            }
            return recorder
        }

        private func record(id: String) {
            Task {
                guard await AVAudioApplication.requestRecordPermission() else {
                    post([
                        "event": "record_failed", "id": id,
                        "message": "Lockbook can't use the microphone. Turn it on in Settings > Lockbook.",
                    ])
                    return
                }
                player?.stop()
                startingID = id
                do {
                    let recorder = try await Task.detached(priority: .userInitiated) {
                        try Self.startRecorder()
                    }.value
                    guard startingID == id else {
                        recorder.stop()
                        try? FileManager.default.removeItem(at: recorder.url)
                        return
                    }
                    startingID = nil
                    self.recorder = recorder
                    self.recordingID = id
                    if stopWhenStarted.remove(id) != nil {
                        stopRecording(id: id)
                    }
                } catch {
                    if startingID == id { startingID = nil }
                    stopWhenStarted.remove(id)
                    post(["event": "record_failed", "id": id, "message": error.localizedDescription])
                }
            }
        }

        private func stopRecording(id: String) {
            if startingID == id {
                stopWhenStarted.insert(id)
                return
            }
            guard recordingID == id, let recorder else { return }
            let url = recorder.url
            recorder.stop()
            self.recorder = nil
            recordingID = nil
            var durationMs = 0
            if let file = try? AVAudioFile(forReading: url) {
                durationMs = Int(Double(file.length) / file.processingFormat.sampleRate * 1000)
            }
            post(["event": "recorded", "id": id, "path": url.path, "duration_ms": durationMs])
        }

        private func play(id: String, path: String, fromMs: UInt64) {
            guard recorder == nil, startingID == nil else {
                post(["event": "playback_failed", "id": id, "message": "Stop recording before playing."])
                return
            }
            Task {
                do {
                    try await Task.detached(priority: .userInitiated) {
                        try Self.activateSession(recording: false)
                    }.value
                    if player == nil || playerID != id || playerPath != path {
                        player?.stop()
                        let fresh = try AVAudioPlayer(contentsOf: URL(fileURLWithPath: path))
                        fresh.prepareToPlay()
                        player = fresh
                        playerID = id
                        playerPath = path
                    }
                    guard let player else { return }
                    player.currentTime = Double(fromMs) / 1000
                    guard player.play() else { throw VoiceError("The recording couldn't play.") }
                } catch {
                    post(["event": "playback_failed", "id": id, "message": error.localizedDescription])
                }
            }
        }

        private func release(id: String) {
            if startingID == id { startingID = nil }
            stopWhenStarted.remove(id)
            if recordingID == id, let recorder {
                recorder.stop()
                try? FileManager.default.removeItem(at: recorder.url)
                self.recorder = nil
                recordingID = nil
            }
            if playerID == id {
                player?.stop()
                player = nil
                playerID = nil
                playerPath = nil
                reportedPlaying = false
            }
            transcriptions.removeValue(forKey: id)?.cancel()
        }

        private func transcribe(id: String, path: String) {
            transcriptions[id]?.cancel()
            transcriptionFraction[id] = 0
            transcriptions[id] = Task {
                await runTranscription(id: id, path: path)
                transcriptions[id] = nil
                transcriptionFraction[id] = nil
            }
        }

        private func progress(_ id: String, phase: String, fraction: Double, preview: String? = nil) {
            var event: [String: Any] = [
                "event": "transcribing", "id": id, "phase": phase, "fraction": fraction,
            ]
            if let preview { event["preview"] = preview }
            post(event)
        }

        private func advance(_ id: String, to fraction: Double, preview: String? = nil) {
            let best = max(transcriptionFraction[id] ?? 0, min(1, fraction))
            transcriptionFraction[id] = best
            progress(id, phase: "transcribing", fraction: best, preview: preview)
        }

        private func runTranscription(id: String, path: String) async {
            do {
                progress(id, phase: "preparing", fraction: 0)
                guard SpeechTranscriber.isAvailable else {
                    throw VoiceError("On-device transcription isn't available on this device.")
                }
                let locale = await SpeechTranscriber.supportedLocale(equivalentTo: Locale.current)
                    ?? Locale(identifier: "en_US")
                let transcriber = SpeechTranscriber(
                    locale: locale,
                    transcriptionOptions: [],
                    reportingOptions: [],
                    attributeOptions: [.audioTimeRange]
                )

                if let install = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
                    let watcher = Task {
                        while !Task.isCancelled {
                            progress(id, phase: "downloading", fraction: install.progress.fractionCompleted)
                            try? await Task.sleep(for: .milliseconds(150))
                        }
                    }
                    defer { watcher.cancel() }
                    try await install.downloadAndInstall()
                }

                let file = try AVAudioFile(forReading: URL(fileURLWithPath: path))
                let duration = max(0.001, Double(file.length) / file.processingFormat.sampleRate)

                let options: SpeechAnalyzer.Options
                if #available(iOS 27, *) {
                    options = SpeechAnalyzer.Options(
                        priority: .userInitiated, modelRetention: .processLifetime, ignoresResourceLimits: true
                    )
                } else {
                    options = SpeechAnalyzer.Options(priority: .userInitiated, modelRetention: .processLifetime)
                }
                let clock = ContinuousClock()
                let started = clock.now
                let analyzer = SpeechAnalyzer(modules: [transcriber], options: options)
                try await analyzer.prepareToAnalyze(in: file.processingFormat)
                advance(id, to: 0)

                let collector = Task { () throws -> [[[String: Any]]] in
                    var segments: [[[String: Any]]] = []
                    var text = ""
                    for try await result in transcriber.results {
                        var words: [[String: Any]] = []
                        for run in result.text.runs {
                            let piece = String(result.text[run.range].characters)
                            guard let range = run.audioTimeRange else {
                                if !piece.isEmpty, var last = words.popLast() {
                                    last["text"] = (last["text"] as? String ?? "") + piece
                                    words.append(last)
                                }
                                continue
                            }
                            words.append([
                                "text": piece,
                                "start_ms": Int(max(0, range.start.seconds) * 1000),
                                "end_ms": Int(max(0, range.end.seconds) * 1000),
                            ])
                        }
                        if !words.isEmpty { segments.append(words) }
                        text += String(result.text.characters)
                        advance(id, to: result.range.end.seconds / duration, preview: previewTail(text))
                    }
                    return segments
                }

                if let last = try await analyzer.analyzeSequence(from: file) {
                    try await analyzer.finalizeAndFinish(through: last)
                } else {
                    await analyzer.cancelAndFinishNow()
                }
                let segments = try await collector.value
                let took = started.duration(to: clock.now)
                let tookMs = Int(took.components.seconds * 1000) + Int(took.components.attoseconds / 1_000_000_000_000_000)

                post([
                    "event": "transcribed", "id": id,
                    "transcript": [
                        "engine": "apple-speech-transcriber",
                        "locale": locale.identifier,
                        "took_ms": tookMs,
                        "segments": segments.map { ["words": $0] },
                    ],
                ])
            } catch is CancellationError {
            } catch {
                post(["event": "transcribe_failed", "id": id, "message": error.localizedDescription])
            }
        }
    }

    private func previewTail(_ text: String) -> String {
        guard text.count > 400 else { return text }
        let tail = text.suffix(400)
        guard let space = tail.firstIndex(of: " ") else { return String(tail) }
        return String(tail[tail.index(after: space)...])
    }

    private struct VoiceError: LocalizedError {
        let message: String
        init(_ message: String) { self.message = message }
        var errorDescription: String? { message }
    }
#endif
