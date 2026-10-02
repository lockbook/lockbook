import AVFoundation
import Bridge
import Foundation

/// The audio behind a chat's spoken conversation. The microphone goes up to
/// the workspace as PCM16 mono at 24 kHz with the replies cancelled out of
/// it; the replies come down the same way and are played, and how far each
/// has played is reported back, since that is where the chat cuts a reply
/// the user spoke over.
final class VoiceEngine {
    static let shared = VoiceEngine()

    private let engine = AVAudioEngine()
    private let player = AVAudioPlayerNode()
    private let wire = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 24000, channels: 1, interleaved: true)!
    private let playback = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 24000, channels: 1, interleaved: false)!
    /// Frames of a reply scheduled at a time: 40 ms, which is how finely
    /// what has played is reported.
    private let slice = 960
    private var timer: Timer?
    /// The workspace whose call this is.
    private var handle: UnsafeMutableRawPointer?
    /// A call was asked for and has not ended; the microphone may still be
    /// being asked for.
    private var wanted = false
    private var running = false
    /// Bumped on every flush and stop, so a buffer scheduled before one does
    /// not report itself played.
    private var generation = 0
    private var playedBytes: [UInt32: Int] = [:]
    private var observers: [NSObjectProtocol] = []

    private init() {
        engine.attach(player)
        engine.connect(player, to: engine.mainMixerNode, format: playback)
        let center = NotificationCenter.default
        observers.append(center.addObserver(forName: .AVAudioEngineConfigurationChange, object: engine, queue: .main) { [weak self] _ in
            self?.recover()
        })
        #if os(iOS)
            observers.append(center.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] note in
                let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt
                if raw.flatMap(AVAudioSession.InterruptionType.init) == .ended {
                    self?.recover()
                }
            })
        #endif
    }

    /// Takes what the workspace has for the engine. Called after every
    /// frame, and every 40 ms while a call is on.
    func service(_ wsHandle: UnsafeMutableRawPointer?) {
        guard let wsHandle else { return }
        let out = voice_take(wsHandle)
        if out.start { start(wsHandle) }
        if out.flush { flush() }
        if let pcm = dataFromBytes(b: out.pcm) { play(reply: out.reply, pcm: pcm) }
        if out.stop { stop() }
    }

    /// The workspace is going away, and with it any call of its.
    func release(_ wsHandle: UnsafeMutableRawPointer?) {
        if wsHandle != nil, wsHandle == handle { stop() }
    }

    private func start(_ wsHandle: UnsafeMutableRawPointer) {
        handle = wsHandle
        wanted = true
        requestMicrophone { [self] granted in
            guard wanted else { return }
            guard granted else {
                print("voice | microphone refused")
                voice_hang_up()
                return
            }
            do {
                try run()
                let timer = Timer(timeInterval: 0.04, repeats: true) { [weak self] _ in
                    self?.service(wsHandle)
                }
                RunLoop.main.add(timer, forMode: .common)
                self.timer?.invalidate()
                self.timer = timer
            } catch {
                print("voice | engine failed: \(error)")
                voice_hang_up()
            }
        }
    }

    private func run() throws {
        #if os(iOS)
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.playAndRecord, mode: .voiceChat, options: [.defaultToSpeaker, .allowBluetoothHFP])
            try session.setActive(true)
        #endif
        let input = engine.inputNode
        if !input.isVoiceProcessingEnabled {
            try input.setVoiceProcessingEnabled(true)
        }
        try listen()
        engine.prepare()
        try engine.start()
        player.play()
        running = true
    }

    /// Taps the microphone as it is now, each tap converting to the wire
    /// with a converter of its own.
    private func listen() throws {
        let input = engine.inputNode
        input.removeTap(onBus: 0)
        let heard = input.outputFormat(forBus: 0)
        guard heard.sampleRate > 0, heard.channelCount > 0, let converter = AVAudioConverter(from: heard, to: wire) else {
            throw VoiceError.noMicrophone
        }
        let wire = self.wire
        input.installTap(onBus: 0, bufferSize: 2400, format: heard) { buffer, _ in
            VoiceEngine.send(buffer, through: converter, as: wire)
        }
    }

    /// The hardware changed under the engine, or an interruption ended:
    /// carries on with the microphone as it is now.
    private func recover() {
        guard running else { return }
        do {
            #if os(iOS)
                try AVAudioSession.sharedInstance().setActive(true)
            #endif
            try listen()
            if !engine.isRunning {
                engine.prepare()
                try engine.start()
            }
            player.play()
        } catch {
            print("voice | could not recover: \(error)")
            voice_hang_up()
        }
    }

    private func stop() {
        wanted = false
        handle = nil
        timer?.invalidate()
        timer = nil
        generation += 1
        playedBytes.removeAll()
        guard running else { return }
        running = false
        player.stop()
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        #if os(iOS)
            try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
        #endif
    }

    /// The user spoke over the reply: what is queued is dropped.
    private func flush() {
        generation += 1
        guard running, engine.isRunning else { return }
        player.stop()
        player.play()
    }

    private func play(reply: UInt32, pcm: Data) {
        guard running, engine.isRunning else { return }
        let frames = pcm.count / 2
        var from = 0
        while from < frames {
            let count = min(slice, frames - from)
            guard let buffer = AVAudioPCMBuffer(pcmFormat: playback, frameCapacity: AVAudioFrameCount(count)) else { return }
            buffer.frameLength = AVAudioFrameCount(count)
            let out = buffer.floatChannelData![0]
            pcm.withUnsafeBytes { raw in
                for i in 0 ..< count {
                    out[i] = Float(raw.loadUnaligned(fromByteOffset: (from + i) * 2, as: Int16.self)) / 32768
                }
            }
            let scheduled = generation
            let bytes = count * 2
            player.scheduleBuffer(buffer, completionCallbackType: .dataPlayedBack) { [weak self] _ in
                DispatchQueue.main.async {
                    guard let self, scheduled == self.generation else { return }
                    let played = (self.playedBytes[reply] ?? 0) + bytes
                    self.playedBytes[reply] = played
                    voice_played(reply, UInt64(played / 48))
                }
            }
            from += count
        }
    }

    /// Microphone audio, as the hardware gives it, sent up as the wire takes it.
    private static func send(_ buffer: AVAudioPCMBuffer, through converter: AVAudioConverter, as wire: AVAudioFormat) {
        let frames = AVAudioFrameCount(Double(buffer.frameLength) * wire.sampleRate / buffer.format.sampleRate) + 16
        guard let out = AVAudioPCMBuffer(pcmFormat: wire, frameCapacity: frames) else { return }
        var given = false
        var error: NSError?
        converter.convert(to: out, error: &error) { _, status in
            if given {
                status.pointee = .noDataNow
                return nil
            }
            given = true
            status.pointee = .haveData
            return buffer
        }
        guard error == nil, out.frameLength > 0, let samples = out.int16ChannelData else { return }
        let count = Int(out.frameLength) * 2
        samples[0].withMemoryRebound(to: UInt8.self, capacity: count) { bytes in
            voice_audio(bytes, UInt(count))
        }
    }

    private func requestMicrophone(_ then: @escaping (Bool) -> Void) {
        #if os(iOS)
            AVAudioApplication.requestRecordPermission { granted in
                DispatchQueue.main.async { then(granted) }
            }
        #else
            AVCaptureDevice.requestAccess(for: .audio) { granted in
                DispatchQueue.main.async { then(granted) }
            }
        #endif
    }
}

private enum VoiceError: Error {
    case noMicrophone
}
