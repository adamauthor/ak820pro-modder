import CoreAudio
import Foundation

enum MusicError: Error, CustomStringConvertible {
    case failed(String)
    var description: String { if case .failed(let message) = self { return message }; return "Unknown error" }
}

// Reads Core Audio activity flags only. No audio recording, taps or private APIs.
func audioProcesses() throws -> [AudioObjectID] {
    var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyProcessObjectList, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size) == noErr else {
        throw MusicError.failed("Cannot read Core Audio process list")
    }
    var objects = [AudioObjectID](repeating: 0, count: Int(size) / MemoryLayout<AudioObjectID>.size)
    guard size > 0 else { return [] }
    let result = objects.withUnsafeMutableBytes { AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, $0.baseAddress!) }
    guard result == noErr else { throw MusicError.failed("Cannot read Core Audio processes") }
    return Array(objects.prefix(Int(size) / MemoryLayout<AudioObjectID>.size))
}

func outputIsActive() throws -> Bool {
    guard #available(macOS 14.2, *) else { throw MusicError.failed("YouTube Music fallback requires macOS 14.2 or later") }
    for process in try audioProcesses() {
        var address = AudioObjectPropertyAddress(mSelector: kAudioProcessPropertyIsRunningOutput, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        var value: UInt32 = 0
        var size = UInt32(MemoryLayout<UInt32>.size)
        let result = AudioObjectGetPropertyData(process, &address, 0, nil, &size, &value)
        // An exiting process can disappear between the list and the property read.
        if result == kAudioHardwareBadObjectError { continue }
        guard result == noErr else { throw MusicError.failed("Cannot determine audio output state") }
        if value != 0 { return true }
    }
    return false
}

enum MusicAction: Equatable { case pauseYouTube, pauseOtherPlayer, resumeYouTube, startYouTube }
func musicAction(audioActive: Bool?, youtubePlaying: Bool?, otherPlayerPlaying: Bool) -> MusicAction {
    if youtubePlaying == true { return .pauseYouTube }
    if otherPlayerPlaying && audioActive != false { return .pauseOtherPlayer }
    if youtubePlaying == false { return .resumeYouTube }
    // An absent YT window is not a paused system media session. Never send a
    // generic media key here: macOS can launch Music for that stale session.
    return .startYouTube
}
