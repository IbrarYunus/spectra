import Foundation
import ScreenCaptureKit
import AVFoundation
import CoreMedia

public typealias SpectraAudioCallback = @convention(c) (
    UnsafePointer<Float>?, Int32, Int32, UnsafeMutableRawPointer?
) -> Void

final class SpectraStreamHandler: NSObject, SCStreamOutput, SCStreamDelegate {
    let callback: SpectraAudioCallback
    let ctx: UnsafeMutableRawPointer?

    init(cb: @escaping SpectraAudioCallback, ctx: UnsafeMutableRawPointer?) {
        self.callback = cb
        self.ctx = ctx
    }

    func stream(_ stream: SCStream,
                didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
                of outputType: SCStreamOutputType) {
        guard outputType == .audio else { return }
        guard CMSampleBufferIsValid(sampleBuffer) else { return }
        guard let formatDesc = CMSampleBufferGetFormatDescription(sampleBuffer),
              let asbdPtr = CMAudioFormatDescriptionGetStreamBasicDescription(formatDesc)
        else { return }
        let channels = Int(asbdPtr.pointee.mChannelsPerFrame)

        var sizeNeeded = 0
        CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
            sampleBuffer,
            bufferListSizeNeededOut: &sizeNeeded,
            bufferListOut: nil,
            bufferListSize: 0,
            blockBufferAllocator: nil,
            blockBufferMemoryAllocator: nil,
            flags: 0,
            blockBufferOut: nil
        )
        guard sizeNeeded > 0 else { return }

        let raw = UnsafeMutableRawPointer.allocate(byteCount: sizeNeeded, alignment: 16)
        defer { raw.deallocate() }
        let ablPtr = raw.assumingMemoryBound(to: AudioBufferList.self)

        var blockBuffer: CMBlockBuffer?
        let status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
            sampleBuffer,
            bufferListSizeNeededOut: nil,
            bufferListOut: ablPtr,
            bufferListSize: sizeNeeded,
            blockBufferAllocator: nil,
            blockBufferMemoryAllocator: nil,
            flags: kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment,
            blockBufferOut: &blockBuffer
        )
        guard status == noErr else { return }

        let abl = UnsafeMutableAudioBufferListPointer(ablPtr)
        guard abl.count > 0 else { return }
        let frameCount = Int(abl[0].mDataByteSize) / MemoryLayout<Float>.size
        guard frameCount > 0 else { return }

        if abl.count >= 2 && channels >= 2 {
            let l = abl[0].mData!.assumingMemoryBound(to: Float.self)
            let r = abl[1].mData!.assumingMemoryBound(to: Float.self)
            var mono = [Float](repeating: 0, count: frameCount)
            for i in 0..<frameCount {
                mono[i] = (l[i] + r[i]) * 0.5
            }
            mono.withUnsafeBufferPointer { ptr in
                callback(ptr.baseAddress, Int32(frameCount), 1, ctx)
            }
        } else {
            let data = abl[0].mData!.assumingMemoryBound(to: Float.self)
            callback(data, Int32(frameCount), Int32(channels), ctx)
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        NSLog("spectra: SCStream stopped: \(error.localizedDescription)")
    }
}

final class StartResult {
    var rc: Int32 = 0
    var errCode: Int32 = 0
    var sampleRate: Int32 = 0
    var stream: SCStream?
    var handler: SpectraStreamHandler?
}

final class SpectraSC {
    static var shared: SpectraSC?
    var stream: SCStream?
    var handler: SpectraStreamHandler?

    func start(_ cb: @escaping SpectraAudioCallback,
               _ ctx: UnsafeMutableRawPointer?,
               _ srOut: UnsafeMutablePointer<Int32>,
               _ errOut: UnsafeMutablePointer<Int32>) -> Int32 {
        let sem = DispatchSemaphore(value: 0)
        let result = StartResult()
        let requestedSR: Int32 = 48000

        Task.detached {
            do {
                let content = try await SCShareableContent.current
                guard let display = content.displays.first else {
                    result.rc = -1; result.errCode = -1; sem.signal(); return
                }
                let filter = SCContentFilter(display: display, excludingWindows: [])
                let config = SCStreamConfiguration()
                config.capturesAudio = true
                config.sampleRate = Int(requestedSR)
                config.channelCount = 2
                config.excludesCurrentProcessAudio = true
                config.width = 2
                config.height = 2
                config.minimumFrameInterval = CMTime(value: 1, timescale: 1)
                config.queueDepth = 5

                let handler = SpectraStreamHandler(cb: cb, ctx: ctx)
                let stream = SCStream(filter: filter, configuration: config, delegate: handler)
                try stream.addStreamOutput(
                    handler,
                    type: .audio,
                    sampleHandlerQueue: DispatchQueue.global(qos: .userInteractive)
                )
                try await stream.startCapture()
                result.stream = stream
                result.handler = handler
                result.sampleRate = requestedSR
            } catch {
                let ns = error as NSError
                NSLog("spectra: start failed: \(ns.domain) \(ns.code) \(ns.localizedDescription)")
                result.rc = -2
                result.errCode = Int32(truncatingIfNeeded: ns.code)
            }
            sem.signal()
        }
        sem.wait()
        self.stream = result.stream
        self.handler = result.handler
        if result.rc == 0 {
            srOut.pointee = result.sampleRate
        }
        errOut.pointee = result.errCode
        return result.rc
    }

    func stop() {
        if let s = self.stream {
            let sem = DispatchSemaphore(value: 0)
            Task {
                try? await s.stopCapture()
                sem.signal()
            }
            _ = sem.wait(timeout: .now() + .seconds(1))
        }
        self.stream = nil
        self.handler = nil
    }
}

@_cdecl("spectra_sc_start")
public func spectra_sc_start(_ cb: SpectraAudioCallback,
                             _ ctx: UnsafeMutableRawPointer?,
                             _ srOut: UnsafeMutablePointer<Int32>,
                             _ errOut: UnsafeMutablePointer<Int32>) -> Int32 {
    let inst = SpectraSC()
    SpectraSC.shared = inst
    return inst.start(cb, ctx, srOut, errOut)
}

@_cdecl("spectra_sc_stop")
public func spectra_sc_stop() {
    SpectraSC.shared?.stop()
    SpectraSC.shared = nil
}
