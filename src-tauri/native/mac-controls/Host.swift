// Host integrations that feed the Rust side: events, notifications, system
// media keys, microphone mute and the Teams/Telegram detectors.
import AppKit
import ApplicationServices
import CoreAudio
import CoreBluetooth
import Foundation
import UserNotifications

// MARK: Events to Rust

public typealias EventCallback = @convention(c) (UnsafePointer<CChar>) -> Void
private var eventCallback: EventCallback?

@_cdecl("ak820_mac_set_event_callback")
public func ak820MacSetEventCallback(_ callback: EventCallback?) { eventCallback = callback }

/// Delivers one JSON event to Rust. Rust copies the string before returning.
func emit(_ event: [String: Any]) {
    guard let callback = eventCallback, let text = try? encodeJSON(event) else { return }
    text.withCString { callback($0) }
}

// MARK: Notifications

func postNotification(title: String, body: String) {
    let center = UNUserNotificationCenter.current()
    center.requestAuthorization(options: [.alert, .sound]) { granted, _ in
        guard granted else { return }
        let content = UNMutableNotificationContent()
        content.title = title; content.body = body; content.sound = .default
        center.add(UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil))
    }
}

// MARK: System media keys

private let soundUp: Int32 = 0, soundDown: Int32 = 1, muteKey: Int32 = 7

/// Posts the same system-defined event as the Mac's own volume keys, so
/// macOS shows its volume indicator, uses its step size and unmutes on change.
func postMediaKey(_ key: Int32) -> Bool {
    guard AXIsProcessTrusted(), CGPreflightPostEventAccess() else { return false }
    for down in [true, false] {
        let flags = NSEvent.ModifierFlags(rawValue: down ? 0xa00 : 0xb00)
        guard let event = NSEvent.otherEvent(with: .systemDefined, location: .zero, modifierFlags: flags, timestamp: 0,
                                             windowNumber: 0, context: nil, subtype: 8,
                                             data1: Int((key << 16) | ((down ? 0xa : 0xb) << 8)), data2: -1),
              let cgEvent = event.cgEvent else { return false }
        cgEvent.post(tap: .cghidEventTap)
    }
    return true
}
func stepVolume(_ direction: Int) throws {
    if postMediaKey(direction > 0 ? soundUp : soundDown) { return }
    try changeOutputVolume(direction > 0 ? 0.0625 : -0.0625)
}
func toggleOutputMute() throws {
    guard postMediaKey(muteKey) else { throw MusicError.failed("Enable Accessibility for AK820 Pro Modder") }
}

private func defaultDevice(_ selector: AudioObjectPropertySelector) -> AudioDeviceID? {
    var address = AudioObjectPropertyAddress(mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    var device = AudioDeviceID(0), size = UInt32(MemoryLayout<AudioDeviceID>.size)
    guard AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &device) == noErr, device != 0 else { return nil }
    return device
}
private var savedInputVolume: Float32?

/// Toggles the default microphone. Uses the device mute switch when present,
/// otherwise input volume 0 with the previous level remembered.
@discardableResult
func toggleMicrophoneMute() throws -> Bool {
    guard let device = defaultDevice(kAudioHardwarePropertyDefaultInputDevice) else { throw MusicError.failed("No microphone is selected") }
    var address = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyMute, mScope: kAudioDevicePropertyScopeInput, mElement: kAudioObjectPropertyElementMain)
    var settable = DarwinBoolean(false)
    if AudioObjectIsPropertySettable(device, &address, &settable) == noErr, settable.boolValue {
        var muted: UInt32 = 0, size = UInt32(MemoryLayout<UInt32>.size)
        _ = AudioObjectGetPropertyData(device, &address, 0, nil, &size, &muted)
        muted = muted == 0 ? 1 : 0
        guard AudioObjectSetPropertyData(device, &address, 0, nil, size, &muted) == noErr else { throw MusicError.failed("Could not change the microphone") }
        return muted == 1
    }
    address.mSelector = kAudioDevicePropertyVolumeScalar
    guard AudioObjectIsPropertySettable(device, &address, &settable) == noErr, settable.boolValue else { throw MusicError.failed("This microphone cannot be muted from macOS") }
    var volume: Float32 = 0, size = UInt32(MemoryLayout<Float32>.size)
    _ = AudioObjectGetPropertyData(device, &address, 0, nil, &size, &volume)
    let mute = volume > 0
    if mute { savedInputVolume = volume }
    var next: Float32 = mute ? 0 : (savedInputVolume ?? 0.75)
    guard AudioObjectSetPropertyData(device, &address, 0, nil, size, &next) == noErr else { throw MusicError.failed("Could not change the microphone") }
    return mute
}

// MARK: Detectors

/// True while a Microsoft Teams process is capturing audio, i.e. in a call.
func teamsInCall() -> Bool {
    guard #available(macOS 14.2, *), let processes = try? audioProcesses() else { return false }
    for process in processes {
        var address = AudioObjectPropertyAddress(mSelector: kAudioProcessPropertyBundleID, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        var bundle: Unmanaged<CFString>?
        var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        guard AudioObjectGetPropertyData(process, &address, 0, nil, &size, &bundle) == noErr,
              let id = bundle?.takeRetainedValue() as String?, isTeamsBundle(id) else { continue }
        address.mSelector = kAudioProcessPropertyIsRunningInput
        var running: UInt32 = 0
        size = UInt32(MemoryLayout<UInt32>.size)
        if AudioObjectGetPropertyData(process, &address, 0, nil, &size, &running) == noErr, running != 0 { return true }
    }
    return false
}
func isTeamsBundle(_ id: String) -> Bool { id.lowercased().hasPrefix("com.microsoft.teams") }

let telegramNames: Set<String> = ["Telegram", "Telegram Desktop", "Telegram Lite"]
/// Unread count from Telegram's Dock badge, read through Accessibility.
/// nil when Telegram is not in the Dock or Accessibility is missing.
func telegramBadge() -> Int? {
    if let badge = launchServicesBadge() { return badge }
    guard AXIsProcessTrusted(), let dock = NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.dock").first else { return nil }
    let root = AXUIElementCreateApplication(dock.processIdentifier)
    AXUIElementSetMessagingTimeout(root, 0.5)
    guard let list = axChildren(root).first(where: { axRole($0) == "AXList" }) else { return nil }
    for item in axChildren(list) where telegramNames.contains(axAttribute(item, kAXTitleAttribute) as? String ?? "") {
        return badgeCount(axAttribute(item, "AXStatusLabel") as? String)
    }
    return nil
}
/// The Dock badge as Launch Services reports it; needs no Accessibility.
func launchServicesBadge() -> Int? {
    for bundle in ["ru.keepcoder.Telegram", "org.telegram.desktop", "com.tdesktop.Telegram"] {
        guard NSRunningApplication.runningApplications(withBundleIdentifier: bundle).first != nil else { continue }
        let process = Process(), pipe = Pipe()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/lsappinfo")
        process.arguments = ["info", "-only", "StatusLabel", "-app", bundle]
        process.standardOutput = pipe; process.standardError = FileHandle.nullDevice
        guard (try? process.run()) != nil else { continue }
        let text = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        process.waitUntilExit()
        return badgeCount(parseStatusLabel(text))
    }
    return nil
}
/// `"StatusLabel"={ "label"="3" }` → "3"; kCFNULL or missing → nil.
func parseStatusLabel(_ text: String) -> String? {
    guard let range = text.range(of: "\"label\"=\"") else { return nil }
    let rest = text[range.upperBound...]
    guard let end = rest.firstIndex(of: "\"") else { return nil }
    return String(rest[..<end])
}
func badgeCount(_ label: String?) -> Int {
    guard let label, !label.isEmpty else { return 0 }
    let digits = label.filter(\.isNumber)
    return Int(digits) ?? 1 // "•" or other non-numeric badges still mean unread
}

let teamsAnswerButtons: Set<String> = ["accept", "accept call", "accept with video", "accept with audio", "decline", "decline call",
                                       "принять", "принять вызов", "принять с видео", "принять с аудио", "отклонить", "отклонить вызов"]
func isTeamsApp(_ app: NSRunningApplication) -> Bool { isTeamsBundle(app.bundleIdentifier ?? "") }

/// An incoming Teams call shows a small window with Accept/Decline buttons.
/// With Accessibility the buttons are matched by name; without it a small,
/// floating, on-screen Teams window is taken as the ringing toast.
func teamsRinging() -> Bool {
    let teams = NSWorkspace.shared.runningApplications.filter(isTeamsApp)
    guard !teams.isEmpty else { return false }
    if AXIsProcessTrusted() {
        for app in teams {
            let root = AXUIElementCreateApplication(app.processIdentifier)
            AXUIElementSetMessagingTimeout(root, 0.3)
            for window in axAttribute(root, kAXWindowsAttribute) as? [AXUIElement] ?? [] where isToastSized(window) {
                if axFind(window, limit: 400, seconds: 0.4, where: { axRole($0) == kAXButtonRole && teamsAnswerButtons.contains(axName($0).lowercased()) }) != nil { return true }
            }
        }
        return false
    }
    let pids = Set(teams.map(\.processIdentifier))
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
    return windows.contains { window in
        guard let pid = window[kCGWindowOwnerPID as String] as? pid_t, pids.contains(pid),
              let layer = window[kCGWindowLayer as String] as? Int, layer > 0, layer < 25,
              let bounds = window[kCGWindowBounds as String] as? [String: Double],
              let width = bounds["Width"], let height = bounds["Height"] else { return false }
        return (250...720).contains(width) && (80...480).contains(height)
    }
}

/// The call toast is small; skipping big windows keeps the check cheap.
func isToastSized(_ window: AXUIElement) -> Bool {
    guard let value = axAttribute(window, kAXSizeAttribute), CFGetTypeID(value) == AXValueGetTypeID() else { return false }
    var size = CGSize.zero
    guard AXValueGetValue(unsafeBitCast(value, to: AXValue.self), .cgSize, &size) else { return false }
    return size.width <= 720 && size.height <= 480
}

/// Number of Telegram banners on screen. Only banner-sized windows count:
/// the open Notification Center list and widgets mention Telegram too and
/// change their texts ("now", "1m ago") all the time.
func telegramBannerCount() -> Int {
    guard AXIsProcessTrusted(), let center = NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.notificationcenterui").first else { return 0 }
    let root = AXUIElementCreateApplication(center.processIdentifier)
    AXUIElementSetMessagingTimeout(root, 0.3)
    var banners = 0
    for window in axAttribute(root, kAXWindowsAttribute) as? [AXUIElement] ?? [] where isBannerSized(window) {
        var stack = [window], texts: [String] = [], count = 0
        while let element = stack.popLast(), count < 150 {
            count += 1
            let name = axName(element)
            if !name.isEmpty { texts.append(name) }
            if let value = axAttribute(element, kAXValueAttribute) as? String, !value.isEmpty { texts.append(value) }
            stack.append(contentsOf: axChildren(element))
        }
        if texts.contains(where: { $0.localizedCaseInsensitiveContains("telegram") }) { banners += 1 }
    }
    return banners
}
func isBannerSized(_ window: AXUIElement) -> Bool {
    guard let value = axAttribute(window, kAXSizeAttribute), CFGetTypeID(value) == AXValueGetTypeID() else { return false }
    var size = CGSize.zero
    guard AXValueGetValue(unsafeBitCast(value, to: AXValue.self), .cgSize, &size) else { return false }
    return size.width <= 520 && size.height <= 260
}

final class Detectors {
    private let queue = DispatchQueue(label: "local.ak820pro.detectors")
    private var timer: DispatchSourceTimer?
    var teams = false, telegram = false
    private var lastCall = false, lastBadge: Int?
    // Teams opens the microphone briefly at launch and for device checks.
    // A call must hold it for a few seconds, and ends after a short silence.
    private var callSince: Date?, quietSince: Date?
    private var lastRinging = false, lastBanners = 0, ticks = 0
    func configure(teams: Bool, telegram: Bool) {
        queue.async { [self] in
            self.teams = teams; self.telegram = telegram
            if !teams && lastCall { lastCall = false; emit(["event": "teams_call", "active": false]) }
            if !teams && lastRinging { lastRinging = false; emit(["event": "teams_ringing", "active": false]) }
            if !telegram { lastBadge = nil }
            if (teams || telegram) && timer == nil {
                let source = DispatchSource.makeTimerSource(queue: queue)
                source.schedule(deadline: .now() + 1, repeating: 1)
                source.setEventHandler { [weak self] in self?.tick() }
                source.resume(); timer = source
            } else if !teams && !telegram { timer?.cancel(); timer = nil }
        }
    }
    private func tick() {
        if teams {
            let now = Date()
            if teamsInCall() {
                quietSince = nil
                if callSince == nil { callSince = now }
            } else {
                callSince = nil
                if quietSince == nil { quietSince = now }
            }
            let call = lastCall
                ? !(quietSince.map { now.timeIntervalSince($0) >= 5 } ?? false)
                : callSince.map { now.timeIntervalSince($0) >= 8 } ?? false
            if call != lastCall { lastCall = call; emit(["event": "teams_call", "active": call]) }
            let ringing = !call && teamsRinging()
            if ringing != lastRinging { lastRinging = ringing; emit(["event": "teams_ringing", "active": ringing]) }
        }
        if telegram {
            let banners = telegramBannerCount()
            if banners > lastBanners { emit(["event": "telegram_message", "unread": -2]) }
            lastBanners = banners
        }
        ticks += 1
        if telegram, ticks % 2 == 0, let badge = telegramBadge() {
            if let lastBadge, badge > lastBadge { emit(["event": "telegram_message", "unread": badge]) }
            lastBadge = badge
        }
    }
}

// MARK: Bluetooth battery

/// Reads the standard BLE Battery Service of an already connected AK820.
/// Never connects to or pairs with anything new.
final class BluetoothBattery: NSObject, CBCentralManagerDelegate, CBPeripheralDelegate {
    private var manager: CBCentralManager?
    private var peripheral: CBPeripheral?
    private var completion: ((Int?) -> Void)?
    private let service = CBUUID(string: "180F"), level = CBUUID(string: "2A19")

    func read(_ done: @escaping (Int?) -> Void) {
        guard completion == nil else { return }
        completion = done
        if let manager, manager.state == .poweredOn { scan(manager) } else if manager == nil { manager = CBCentralManager(delegate: self, queue: .main) }
        DispatchQueue.main.asyncAfter(deadline: .now() + 8) { [weak self] in self?.finish(nil) }
    }
    private func scan(_ central: CBCentralManager) {
        guard let match = central.retrieveConnectedPeripherals(withServices: [service]).first(where: { ($0.name ?? "").hasPrefix("AK820") }) else { finish(nil); return }
        peripheral = match; match.delegate = self
        central.connect(match) // Already connected by macOS; this only opens our GATT client.
    }
    private func finish(_ value: Int?) {
        guard let completion else { return }
        self.completion = nil
        if let peripheral, let manager { manager.cancelPeripheralConnection(peripheral) }
        peripheral = nil
        completion(value)
    }
    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        if central.state == .poweredOn, completion != nil { scan(central) } else if central.state != .unknown && central.state != .resetting { finish(nil) }
    }
    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) { peripheral.discoverServices([service]) }
    func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?) { finish(nil) }
    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        guard let battery = peripheral.services?.first(where: { $0.uuid == service }) else { finish(nil); return }
        peripheral.discoverCharacteristics([level], for: battery)
    }
    func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?) {
        guard let characteristic = service.characteristics?.first(where: { $0.uuid == level }) else { finish(nil); return }
        peripheral.readValue(for: characteristic)
    }
    func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?) {
        guard let byte = characteristic.value?.first, byte <= 100 else { finish(nil); return }
        finish(Int(byte))
    }
}
