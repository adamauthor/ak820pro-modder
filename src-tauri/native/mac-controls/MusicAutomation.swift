import AppKit
import Carbon
import Foundation

func fourCC(_ value: String) -> OSType {
    precondition(value.utf8.count == 4)
    return value.utf8.reduce(0) { ($0 << 8) | UInt32($1) }
}
func nativeMusicBundle(_ service: String) -> String? {
    service == "apple_music" ? "com.apple.Music" : service == "spotify" ? "com.spotify.client" : nil
}
// Build a standard Apple Event property specifier. No scripts, shell commands,
// account tokens or user-supplied application identifiers are evaluated.
func musicProperty(_ code: String, container: NSAppleEventDescriptor = .null()) -> NSAppleEventDescriptor {
    let record = NSAppleEventDescriptor.record()
    record.setDescriptor(NSAppleEventDescriptor(typeCode: fourCC("prop")), forKeyword: AEKeyword(keyAEDesiredClass))
    record.setDescriptor(NSAppleEventDescriptor(enumCode: fourCC("prop")), forKeyword: AEKeyword(keyAEKeyForm))
    record.setDescriptor(NSAppleEventDescriptor(typeCode: fourCC(code)), forKeyword: AEKeyword(keyAEKeyData))
    record.setDescriptor(container, forKeyword: AEKeyword(keyAEContainer))
    return record.coerce(toDescriptorType: OSType(typeObjectSpecifier))!
}
func musicAutomationPermission(_ service: String, prompt: Bool = false) -> String {
    guard let bundle = nativeMusicBundle(service) else { return "not_needed" }
    guard NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundle) != nil else { return "not_installed" }
    let target = NSAppleEventDescriptor(bundleIdentifier: bundle)
    guard let descriptor = target.aeDesc else { return "unavailable" }
    switch AEDeterminePermissionToAutomateTarget(descriptor, OSType(typeWildCard), OSType(typeWildCard), prompt) {
    case noErr: return "allowed"
    case -1744: return "ask"
    case -1743: return "denied"
    default: return "unavailable"
    }
}

struct NativeMusicClient {
    let service: String
    let pid: pid_t
    init?(_ service: String) {
        guard let bundle = nativeMusicBundle(service),
              let running = NSRunningApplication.runningApplications(withBundleIdentifier: bundle).first,
              musicAutomationPermission(service) == "allowed" else { return nil }
        self.service = service; pid = running.processIdentifier
    }
    private func send(_ eventClass: OSType, _ eventID: OSType, object: NSAppleEventDescriptor? = nil, data: NSAppleEventDescriptor? = nil) throws -> NSAppleEventDescriptor {
        // Address a specific running process. This can never launch a default or
        // unrelated music application when a media window has been closed.
        let target = NSAppleEventDescriptor(processIdentifier: pid)
        let event = NSAppleEventDescriptor(eventClass: eventClass, eventID: eventID, targetDescriptor: target, returnID: Int16(kAutoGenerateReturnID), transactionID: Int32(kAnyTransactionID))
        if let object { event.setParam(object, forKeyword: keyDirectObject) }
        if let data { event.setParam(data, forKeyword: AEKeyword(keyAEData)) }
        do {
            let reply = try event.sendEvent(options: [.waitForReply, .neverInteract], timeout: 3)
            if let code = reply.paramDescriptor(forKeyword: keyErrorNumber)?.int32Value, code != 0 {
                throw MusicError.failed("Music app command failed (\(code)); choose a track first")
            }
            return reply.paramDescriptor(forKeyword: keyDirectObject) ?? .null()
        } catch let error as NSError where error.code == -1743 || error.code == -1744 {
            throw MusicError.failed("Allow player control in Music settings")
        }
    }
    func property(_ code: String, container: NSAppleEventDescriptor = .null()) throws -> NSAppleEventDescriptor {
        try send(OSType(kAECoreSuite), OSType(kAEGetData), object: musicProperty(code, container: container))
    }
    func isPlaying() throws -> Bool { try property("pPlS").enumCodeValue == fourCC("kPSP") }
    func perform(_ action: String) throws {
        if action == "seek_forward" || action == "seek_backward" {
            let position = try property("pPos").doubleValue
            let duration = try property("pDur", container: musicProperty("pTrk")).doubleValue
            guard position.isFinite, duration.isFinite, position >= 0, duration > 0 else { throw MusicError.failed("Start a track in the selected player first") }
            let desired = min(duration, max(0, position + (action == "seek_forward" ? 5 : -5)))
            _ = try send(OSType(kAECoreSuite), OSType(kAESetData), object: musicProperty("pPos"), data: NSAppleEventDescriptor(double: desired))
            return
        }
        let command: String
        switch action {
        case "toggle": command = "PlPs"
        case "play": command = "Play"
        case "next": command = "Next"
        case "previous": command = "Prev"
        default: throw MusicError.failed("Unknown music action")
        }
        _ = try send(fourCC(service == "spotify" ? "spfy" : "hook"), fourCC(command))
    }
}
