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
    private var converter: AVAudioConverter?
    private var timer: Timer?
    private var running = false
    /// Bumped on every flush and stop, so a buffer scheduled before one does
    /// not report itself played.
    private var generation = 0
    private var playedBytes: [UInt32: Int] = [:]

    private init() {
        engine.attach(player)
        engine.connect(player, to: engine.mainMixerNode, format: playback)
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

    private func start(_ wsHandle: UnsafeMutableRawPointer) {
        requestMicrophone { [self] granted in
            guard granted else {
                print("voice | microphone refused")
                voice_hang_up()
                return
            }
            do {
                try run()
                timer?.invalidate()
                timer = Timer.scheduledTimer(withTimeInterval: 0.04, repeats: true) { [weak self] _ in
                    self?.service(wsHandle)
                }
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
        let heard = input.outputFormat(forBus: 0)
        converter = AVAudioConverter(from: heard, to: wire)
        input.removeTap(onBus: 0)
        input.installTap(onBus: 0, bufferSize: 2400, format: heard) { [weak self] buffer, _ in
            self?.heard(buffer)
        }
        engine.prepare()
        try engine.start()
        player.play()
        running = true
    }

    private func stop() {
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
        guard running else { return }
        player.stop()
        player.play()
    }

    private func play(reply: UInt32, pcm: Data) {
        let frames = pcm.count / 2
        guard running, frames > 0,
              let buffer = AVAudioPCMBuffer(pcmFormat: playback, frameCapacity: AVAudioFrameCount(frames))
        else { return }
        buffer.frameLength = AVAudioFrameCount(frames)
        let out = buffer.floatChannelData![0]
        pcm.withUnsafeBytes { raw in
            for i in 0 ..< frames {
                out[i] = Float(raw.loadUnaligned(fromByteOffset: i * 2, as: Int16.self)) / 32768
            }
        }
        let scheduled = generation
        player.scheduleBuffer(buffer, completionCallbackType: .dataPlayedBack) { [weak self] _ in
            DispatchQueue.main.async {
                guard let self, scheduled == self.generation else { return }
                let played = (self.playedBytes[reply] ?? 0) + pcm.count
                self.playedBytes[reply] = played
                voice_played(reply, UInt64(played / 48))
            }
        }
    }

    /// Microphone audio, as the hardware gives it, sent up as the wire takes it.
    private func heard(_ buffer: AVAudioPCMBuffer) {
        guard let converter else { return }
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
        samples[0].withMemoryRebound(to: UInt8.self, capacity: Int(out.frameLength) * 2) { bytes in
            voice_audio(bytes, UInt(out.frameLength) * 2)
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
