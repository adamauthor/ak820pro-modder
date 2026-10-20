// Linked into the Tauri process. There is no helper app, sidecar, delegate or
// additional permission identity. All entry points except the fixture run on
// Tauri's existing AppKit main thread.
import AppKit
import ApplicationServices
import Carbon
import Darwin
import Foundation

struct MacSnapshot: Codable {
    let supported: Bool
    let youtube_supported: Bool
    let accessibility: Bool
    let hotkey_active: Bool
    let devices: [ConnectedKeyboard]
    let youtube_apps: [YouTubeApp]
    let youtube_app_id: String?
    let message: String
    let error: String?
}

final class NativeControls: NSObject {
    var mode = "original"
    var directory: URL?
    var apps: [YouTubeApp] = []
    var devices: [ConnectedKeyboard] = []
    var message = "Choose the knob action below"
    var lastError: String?
    var timer: Timer?
    var hotkey: EventHotKeyRef?
    var handler: EventHandlerRef?
    var held = false
    let controller = MusicController()
    var statusItem: NSStatusItem?
    var backgroundActivity: NSObjectProtocol?

    override init() {
        super.init()
        controller.status = { [weak self] message in self?.message = message; self?.lastError = nil }
    }

    func configure(_ request: [String: Any]) throws {
        NSApp.setActivationPolicy(.accessory)
        if backgroundActivity == nil {
            backgroundActivity = ProcessInfo.processInfo.beginActivity(options: .userInitiatedAllowingIdleSystemSleep,
                reason: "Respond to AK820 knob and macro shortcuts while the settings window is closed")
        }
        if statusItem == nil {
            let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
            item.button?.title = "AK820"
            item.button?.toolTip = "AK820 Pro Modder"
            let menu = NSMenu()
            let open = NSMenuItem(title: "Open AK820 Pro Modder", action: #selector(openWindow), keyEquivalent: "")
            open.target = self; menu.addItem(open)
            menu.addItem(.separator())
            let quit = NSMenuItem(title: "Stop controls and quit", action: #selector(quitApplication), keyEquivalent: "")
            quit.target = self; menu.addItem(quit)
            item.menu = menu; statusItem = item
        }
        guard let nextMode = request["mode"] as? String, ["original", "play_pause", "youtube_music"].contains(nextMode),
              let path = request["directory"] as? String, path.hasPrefix("/") else { throw MusicError.failed("Invalid Mac control configuration") }
        apps = youtubeApps()
        let selected = request["youtube_app_id"] as? String
        if nextMode == "youtube_music" {
            guard #available(macOS 14.2, *) else { throw MusicError.failed("YouTube Music fallback requires macOS 14.2 or later") }
            guard let app = apps.first(where: { $0.id == selected }) ?? (selected == nil ? apps.first : nil) else {
                throw MusicError.failed("Install YouTube Music as a Safari web app, then refresh")
            }
            youtubeBundle = app.id
        } else { youtubeBundle = selected ?? apps.first?.id ?? "" }
        let previous = mode
        if nextMode != "youtube_music" { controller.cancel() }
        mode = nextMode
        directory = URL(fileURLWithPath: path, isDirectory: true)
        do { try refresh() }
        catch { mode = previous; try? refresh(); throw error }
        if timer == nil {
            let timer = Timer(timeInterval: 10, repeats: true) { [weak self] _ in
                do { try self?.refresh() } catch { self?.lastError = String(describing: error) }
            }
            self.timer = timer
            RunLoop.main.add(timer, forMode: .common)
        }
    }

    func registerHotkey() throws {
        if hotkey != nil { return }
        var types = [EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed)), EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyReleased))]
        let context = Unmanaged.passUnretained(self).toOpaque()
        let result = InstallEventHandler(GetApplicationEventTarget(), { _, event, context -> OSStatus in
            guard let context else { return OSStatus(eventNotHandledErr) }
            var identifier = EventHotKeyID()
            guard GetEventParameter(event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID), nil, MemoryLayout<EventHotKeyID>.size, nil, &identifier) == noErr,
                  identifier.signature == 0x414b3832, identifier.id == 1 else { return OSStatus(eventNotHandledErr) }
            let controls = Unmanaged<NativeControls>.fromOpaque(context).takeUnretainedValue()
            if GetEventKind(event) == UInt32(kEventHotKeyReleased) { controls.held = false }
            else if !controls.held { controls.held = true; controls.controller.handleClick() }
            return noErr
        }, 2, &types, context, &handler)
        guard result == noErr else { throw MusicError.failed("Cannot register the knob handler") }
        let registration = RegisterEventHotKey(UInt32(kVK_F20), 0, EventHotKeyID(signature: 0x414b3832, id: 1), GetApplicationEventTarget(), OptionBits(kEventHotKeyExclusive), &hotkey)
        guard registration == noErr else { unregisterHotkey(); throw MusicError.failed("F20 is already used by another app or automation") }
    }
    func unregisterHotkey() {
        if let hotkey { UnregisterEventHotKey(hotkey) }
        if let handler { RemoveEventHandler(handler) }
        hotkey = nil; handler = nil; held = false
    }
    func refresh() throws {
        guard let directory else { return }
        let trusted = AXIsProcessTrusted() && CGPreflightPostEventAccess()
        let youtube = mode == "youtube_music"
        if youtube { try registerHotkey() }
        do { devices = try updateKnobMappings(target: mode == "original" ? nil : youtube ? triggerUsage : playUsage, directory: directory) }
        catch { if !youtube { unregisterHotkey() }; throw error }
        if !youtube { unregisterHotkey() }
        let needsAccess = mode == "youtube_music" && !trusted
        if needsAccess { message = "Enable Accessibility for AK820 Pro Modder, then Stop controls and quit from the AK820 menu and reopen it" }
        else if devices.isEmpty { message = "Waiting for AK820 over USB or Bluetooth" }
        else if mode == "original" { message = "Previous knob mapping restored" }
        else if lastError != nil || message.hasPrefix("Enable ") || message.hasPrefix("Waiting ") || message.hasPrefix("Previous ") || message.hasPrefix("Choose ") { message = "Ready — USB and Bluetooth" }
        lastError = nil
    }
    func snapshot() -> MacSnapshot {
        let youtubeSupported: Bool
        if #available(macOS 14.2, *) { youtubeSupported = true } else { youtubeSupported = false }
        return MacSnapshot(supported: true, youtube_supported: youtubeSupported, accessibility: AXIsProcessTrusted() && CGPreflightPostEventAccess(), hotkey_active: hotkey != nil, devices: devices, youtube_apps: apps, youtube_app_id: youtubeBundle.isEmpty ? nil : youtubeBundle, message: message, error: lastError)
    }
    func shutdown() throws {
        controller.cancel()
        timer?.invalidate(); timer = nil
        if let backgroundActivity { ProcessInfo.processInfo.endActivity(backgroundActivity) }
        backgroundActivity = nil
        defer { unregisterHotkey() }
        if let directory { _ = try updateKnobMappings(target: nil, directory: directory) }
    }
    @objc func openWindow() {
        NSApp.activate(ignoringOtherApps: true)
        NSApp.windows.first(where: { $0.title == "AK820 Pro Modder" })?.makeKeyAndOrderFront(nil)
    }
    @objc func quitApplication() { NSApp.terminate(nil) }
}
private let controls = NativeControls()

@_cdecl("ak820_mac_controls_request")
public func ak820MacControlsRequest(_ input: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>? {
    do {
        guard Thread.isMainThread else { throw MusicError.failed("Mac controls must run on the main thread") }
        let data = Data(String(cString: input).utf8)
        guard let request = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { throw MusicError.failed("Invalid request") }
        switch request["operation"] as? String {
        case "configure": try controls.configure(request)
        case "refresh": controls.apps = youtubeApps(); try controls.refresh()
        case "press":
            guard controls.mode == "youtube_music" else { throw MusicError.failed("Select YouTube Music mode first") }
            controls.controller.handleClick()
        case "liked":
            guard controls.mode == "youtube_music" else { throw MusicError.failed("Select YouTube Music mode first") }
            controls.controller.playLikedMusic()
        case "shutdown": try controls.shutdown()
        case "status": break
        default: throw MusicError.failed("Unknown Mac control operation")
        }
        return String(decoding: try JSONEncoder().encode(controls.snapshot()), as: UTF8.self).withCString { strdup($0) }
    } catch {
        if Thread.isMainThread { controls.lastError = String(describing: error) }
        let text = (try? encodeJSON(["error": String(describing: error)])) ?? "{\"error\":\"Native operation failed\"}"
        return text.withCString { strdup($0) }
    }
}
@_cdecl("ak820_mac_controls_free")
public func ak820MacControlsFree(_ pointer: UnsafeMutableRawPointer?) { free(pointer) }

@_cdecl("ak820_mac_controls_selftest")
public func ak820MacControlsSelftest() -> Int32 {
    do {
        let key: KeyMapping = [mappingSource: (7 << 32) | 4, mappingDestination: (7 << 32) | 5]
        let (enabled, owner) = try plannedMute([key], owner: nil, target: triggerUsage)
        let (again, againOwner) = try plannedMute(enabled, owner: owner, target: triggerUsage)
        let (restored, noOwner) = try plannedMute(again, owner: againOwner, target: nil)
        guard restored == [key], noOwner == nil, againOwner?.original == nil else { return 1 }
        let custom: KeyMapping = [mappingSource: muteUsage, mappingDestination: playUsage]
        let (changed, backedUp) = try plannedMute([key, custom], owner: nil, target: triggerUsage)
        guard try plannedMute(changed, owner: backedUp, target: nil).0 == [key, custom] else { return 2 }
        guard musicAction(audioActive: true, youtubePlaying: nil, otherPlayerPlaying: false) == .startYouTube,
              musicAction(audioActive: nil, youtubePlaying: nil, otherPlayerPlaying: false) == .startYouTube,
              musicAction(audioActive: false, youtubePlaying: false, otherPlayerPlaying: false) == .resumeYouTube,
              musicAction(audioActive: true, youtubePlaying: true, otherPlayerPlaying: false) == .pauseYouTube,
              musicAction(audioActive: true, youtubePlaying: nil, otherPlayerPlaying: true) == .pauseOtherPlayer,
              musicAction(audioActive: false, youtubePlaying: nil, otherPlayerPlaying: true) == .startYouTube,
              musicAction(audioActive: true, youtubePlaying: true, otherPlayerPlaying: true) == .pauseYouTube else { return 3 }
        let valid: [String: Any] = ["CFBundleIdentifier": "com.apple.Safari.WebApp.test", "Manifest": ["start_url": "https://music.youtube.com/?source=pwa"]]
        guard isYouTubeManifest(valid), !isYouTubeManifest(["CFBundleIdentifier": "com.apple.Safari.WebApp.test", "Manifest": ["start_url": "https://music.youtube.com.evil.example/"]]) else { return 4 }
        do { _ = try plannedMute([custom], owner: MuteOwnership(original: nil, applied: triggerUsage), target: nil); return 5 }
        catch MusicError.failed(let message) where message == "Mute was changed by another app; leaving it unchanged" { }
        return 0
    } catch { return 6 }
}
