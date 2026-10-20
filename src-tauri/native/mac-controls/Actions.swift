import AppKit
import ApplicationServices
import CoreAudio
import Foundation

func changeOutputVolume(_ delta: Float32) throws {
    var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDefaultOutputDevice, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    var device = AudioDeviceID(0), size = UInt32(MemoryLayout<AudioDeviceID>.size)
    guard AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &device) == noErr, device != 0 else { throw MusicError.failed("No output audio device") }
    var changed = false
    for element in [UInt32(0), 1, 2] {
        address = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyVolumeScalar, mScope: kAudioDevicePropertyScopeOutput, mElement: element)
        var settable = DarwinBoolean(false)
        guard AudioObjectIsPropertySettable(device, &address, &settable) == noErr, settable.boolValue else { continue }
        var volume = Float32(0); size = UInt32(MemoryLayout<Float32>.size)
        guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, &volume) == noErr else { continue }
        volume = min(1, max(0, volume + delta))
        guard AudioObjectSetPropertyData(device, &address, 0, nil, size, &volume) == noErr else { throw MusicError.failed("Could not change output volume") }
        changed = true
        if element == 0 { break }
    }
    guard changed else { throw MusicError.failed("This audio output has no software volume control") }
}
func performContextTurn(_ action: String, direction: Int, expectedBundle: String?) throws {
    guard AXIsProcessTrusted(), CGPreflightPostEventAccess() else { throw MusicError.failed("Enable Accessibility for AK820 Pro Modder") }
    guard let expectedBundle, NSWorkspace.shared.frontmostApplication?.bundleIdentifier == expectedBundle,
          expectedBundle != Bundle.main.bundleIdentifier else { throw MusicError.failed("Switch to the target app before using this gesture") }
    guard let source = CGEventSource(stateID: .privateState) else { throw MusicError.failed("Cannot create a keyboard event") }
    if action == "scroll" {
        guard let event = CGEvent(scrollWheelEvent2Source: source, units: .line, wheelCount: 1, wheel1: Int32(-direction * 3), wheel2: 0, wheel3: 0) else { return }
        event.flags = []; event.post(tap: .cghidEventTap); return
    }
    let key: CGKeyCode, flags: CGEventFlags
    switch action {
    case "zoom": key = direction > 0 ? 24 : 27; flags = .maskCommand // = / -
    case "tabs": key = direction > 0 ? 30 : 33; flags = [.maskCommand, .maskShift] // ] / [
    default: return
    }
    guard let down = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: true),
          let up = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: false) else { return }
    down.flags = flags; up.flags = flags
    down.post(tap: .cghidEventTap); up.post(tap: .cghidEventTap)
}

func secondsFromClock(_ value: String) -> Double? {
    let pieces = value.trimmingCharacters(in: .whitespacesAndNewlines).split(separator: ":")
    guard (2...3).contains(pieces.count) else { return nil }
    var seconds: Double = 0
    for (index, part) in pieces.enumerated() {
        guard let number = Double(part), number >= 0, (index == 0 || number < 60) else { return nil }
        seconds = seconds * 60 + number
    }
    return seconds
}

// A normalized progress slider needs elapsed/total time before converting seconds.
// A missing or remaining-time description is deliberately not guessed.
func timelineStep(value: Double, maximum: Double, description: String?, delta: Double) -> Double? {
    guard let description, maximum > 0, value >= 0 else { return nil }
    let lower = description.lowercased()
    guard !lower.contains("remaining"), !lower.contains("осталось"), !lower.contains("−"), !lower.contains("-") else { return nil }
    let expression = try! NSRegularExpression(pattern: #"\b[0-9]{1,2}:[0-9]{2}(?::[0-9]{2})?\b"#)
    let ns = description as NSString
    let times = expression.matches(in: description, range: NSRange(location: 0, length: ns.length)).compactMap { secondsFromClock(ns.substring(with: $0.range)) }
    if times.count == 2, times[1] > 0 { return delta * maximum / times[1] }
    if times.count == 1, times[0] > 0, value > 0 { return delta * value / times[0] }
    return nil
}
