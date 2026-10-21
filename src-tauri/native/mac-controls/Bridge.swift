// All public entry points run on Tauri's AppKit thread. Hardware mappings and
// status-item ownership stay in this process, including after the window closes.
import AppKit
import ApplicationServices
import Carbon
import Darwin
import Foundation

func jsonValue<T: Encodable>(_ value: T) -> Any { (try? JSONSerialization.jsonObject(with: JSONEncoder().encode(value))) ?? NSNull() }
final class NativeControls: NSObject {
    var config: ControlConfig?
    var directory: URL?
    var apps: [YouTubeApp] = []
    var installed: [InstalledApp] = []
    var devices: [ConnectedKeyboard] = []
    var message = "Ready"
    var lastError: String?
    var timer: Timer?
    var hotkeys: [EventHotKeyRef] = []
    var handler: EventHandlerRef?
    var registeredRotation = false
    var heldClick = false
    let controller = MusicController()
    var statusItem: NSStatusItem?
    var backgroundActivity: NSObjectProtocol?
    var applicationObserver: NSObjectProtocol?
    var instanceObserver: NSObjectProtocol?
    var refreshingMappings = false
    var refreshMappingsAgain = false
    var activeBundle: String?
    var activeProfile: AppProfile?
    var pendingClick: DispatchWorkItem?
    var pendingProfile: DispatchWorkItem?
    var sequence = ClickSequence()
    var sequencePreview = false
    var previewUntil: TimeInterval = 0
    var events: [GestureEvent] = []
    var eventID = 0
    var mode: String { config?.knob_mode ?? "original" }
    var advanced: Bool { config?.advanced == true }
    var preview: Bool { ProcessInfo.processInfo.systemUptime < previewUntil }

    override init() {
        super.init()
        controller.status = { [weak self] message in self?.message = message; self?.lastError = message.hasPrefix("Error:") ? message : nil }
    }
    func configure(_ request: [String: Any]) throws {
        guard let object = request["config"], let path = request["directory"] as? String, path.hasPrefix("/") else { throw MusicError.failed("Invalid Mac control configuration") }
        let next = try JSONDecoder().decode(ControlConfig.self, from: JSONSerialization.data(withJSONObject: object))
        try next.validate()
        NSApp.setActivationPolicy(.accessory)
        if backgroundActivity == nil {
            backgroundActivity = ProcessInfo.processInfo.beginActivity(options: .userInitiatedAllowingIdleSystemSleep, reason: "Respond to AK820 controls while the settings window is closed")
        }
        if statusItem == nil {
            let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
            item.button?.title = "AK820"; item.button?.toolTip = "AK820 Pro Modder"
            let menu = NSMenu()
            let open = NSMenuItem(title: "Open AK820 Pro Modder", action: #selector(openWindow), keyEquivalent: "")
            open.target = self; menu.addItem(open); menu.addItem(.separator())
            let quit = NSMenuItem(title: "Stop controls and quit", action: #selector(quitApplication), keyEquivalent: "")
            quit.target = self; menu.addItem(quit); item.menu = menu; statusItem = item
        }
        let previous = config, previousYouTube = youtubeBundle
        cancelClicks(); controller.cancel(); previewUntil = 0
        config = next; directory = URL(fileURLWithPath: path, isDirectory: true)
        apps = youtubeApps(); installed = installedApplications()
        youtubeBundle = (apps.first { $0.id == next.youtube_app_id } ?? (next.youtube_app_id == nil ? apps.first : nil))?.id ?? ""
        controller.service = next.music_service; controller.followPlaying = next.follow_playing
        do { try refresh(force: true); lastError = nil }
        catch {
            config = previous; youtubeBundle = previousYouTube
            controller.service = previous?.music_service ?? "youtube_music"; controller.followPlaying = previous?.follow_playing ?? true
            try? refresh(force: true); throw error
        }
        if applicationObserver == nil {
            applicationObserver = NSWorkspace.shared.notificationCenter.addObserver(forName: NSWorkspace.didActivateApplicationNotification, object: nil, queue: .main) { [weak self] _ in
                do { try self?.refreshContext() } catch { if String(describing: error) != "Release held keys to switch profile mappings" { self?.lastError = String(describing: error) } }
            }
        }
        if instanceObserver == nil {
            instanceObserver = DistributedNotificationCenter.default().addObserver(forName: showSettingsNotification, object: nil, queue: .main) { [weak self] _ in self?.openWindow() }
        }
        if timer == nil {
            let timer = Timer(timeInterval: 10, repeats: true) { [weak self] _ in
                do { try self?.refresh(force: true) } catch { if String(describing: error) != "Release held keys to switch profile mappings" { self?.lastError = String(describing: error) } }
            }
            self.timer = timer; RunLoop.main.add(timer, forMode: .common)
        }
    }
    func registerHotkeys(rotation: Bool) throws {
        if !hotkeys.isEmpty && registeredRotation == rotation { return }
        unregisterHotkeys()
        var types = [EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed)), EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyReleased))]
        let result = InstallEventHandler(GetApplicationEventTarget(), { _, event, context -> OSStatus in
            guard let context else { return OSStatus(eventNotHandledErr) }
            var identifier = EventHotKeyID()
            guard GetEventParameter(event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID), nil, MemoryLayout<EventHotKeyID>.size, nil, &identifier) == noErr,
                  identifier.signature == 0x414b3832, identifier.id >= 1, identifier.id <= 48 else { return OSStatus(eventNotHandledErr) }
            let controls = Unmanaged<NativeControls>.fromOpaque(context).takeUnretainedValue()
            let key = Int((identifier.id - 1) / 16), modifiers = Int((identifier.id - 1) % 16)
            let released = GetEventKind(event) == UInt32(kEventHotKeyReleased)
            if key == 0 {
                if released { controls.heldClick = false }
                else if !controls.heldClick { controls.heldClick = true; controls.click() }
            } else if !released { controls.turn(key == 1 ? 1 : -1, shift: modifiers & 1 != 0) }
            return noErr
        }, 2, &types, Unmanaged.passUnretained(self).toOpaque(), &handler)
        guard result == noErr else { throw MusicError.failed("Cannot register the knob handler") }
        do {
            let keys: [Int] = rotation ? [kVK_F20, kVK_F18, kVK_F19] : [kVK_F20]
            for index in keys.indices {
                let key = keys[index]
                for mask in 0..<16 {
                    var modifiers: UInt32 = 0
                    for (bit, flag) in [shiftKey, optionKey, controlKey, cmdKey].enumerated() where mask & (1 << bit) != 0 { modifiers |= UInt32(flag) }
                    var ref: EventHotKeyRef?
                    guard RegisterEventHotKey(UInt32(key), modifiers, EventHotKeyID(signature: 0x414b3832, id: UInt32(index * 16 + mask + 1)), GetApplicationEventTarget(), OptionBits(kEventHotKeyExclusive), &ref) == noErr, let ref else { throw MusicError.failed("F18–F20 or a modified combination is already used by another app") }
                    hotkeys.append(ref)
                }
            }
            registeredRotation = rotation
        } catch { unregisterHotkeys(); throw error }
    }
    func unregisterHotkeys() {
        for hotkey in hotkeys { UnregisterEventHotKey(hotkey) }
        if let handler { RemoveEventHandler(handler) }
        hotkeys = []; handler = nil; heldClick = false; cancelClicks()
    }
    func refresh(force: Bool = false) throws {
        if advanced { try registerHotkeys(rotation: config?.capture_rotation == true) }
        try refreshContext(force: force)
        if !advanced { unregisterHotkeys() }
        if advanced && !AXIsProcessTrusted() { message = "Enable Accessibility for AK820 Pro Modder, then Stop controls and quit and reopen it" }
        else if devices.isEmpty { message = "Waiting for AK820 over USB or Bluetooth" }
        else if mode == "original" { message = "Previous knob mapping restored" }
        else if message == "Ready" || message.hasPrefix("Waiting") || message.hasPrefix("Previous") { message = "Ready — USB and Bluetooth" }
    }
    func refreshContext(force: Bool = false) throws {
        // waitUntilExit may deliver another activation/timer notification while
        // the current mapping transaction owns its file lock. Coalesce it.
        guard !refreshingMappings else { refreshMappingsAgain = true; return }
        refreshingMappings = true
        defer {
            refreshingMappings = false
            if refreshMappingsAgain {
                refreshMappingsAgain = false
                DispatchQueue.main.async { [weak self] in do { try self?.refreshContext(force: true) } catch { self?.lastError = String(describing: error) } }
            }
        }
        let bundle = NSWorkspace.shared.frontmostApplication?.bundleIdentifier
        let profile = advanced ? config?.profile(for: bundle) : nil
        guard force || bundle != activeBundle || profile != activeProfile else { return }
        if bundle != activeBundle || profile != activeProfile { cancelClicks() }
        if (profile?.remaps ?? []) != (activeProfile?.remaps ?? []),
           (0..<128).contains(where: { ![kVK_F18, kVK_F19, kVK_F20].contains($0) && CGEventSource.keyState(.combinedSessionState, key: CGKeyCode($0)) }) {
            pendingProfile?.cancel()
            let task = DispatchWorkItem { [weak self] in do { try self?.refreshContext(force: true) } catch { if String(describing: error) != "Release held keys to switch profile mappings" { self?.lastError = String(describing: error) } } }
            pendingProfile = task; DispatchQueue.main.asyncAfter(deadline: .now() + 0.08, execute: task)
            throw MusicError.failed("Release held keys to switch profile mappings")
        }
        pendingProfile?.cancel(); pendingProfile = nil
        guard let directory else { return }
        devices = try updateControlMappings(target: mode == "original" ? nil : advanced ? triggerUsage : playUsage,
            rotation: advanced && config?.capture_rotation == true, remaps: profile?.remaps ?? [], directory: directory)
        activeBundle = bundle; activeProfile = profile
        if lastError == "Release held keys to switch profile mappings" { lastError = nil }
    }
    func cancelClicks() { pendingClick?.cancel(); pendingClick = nil; sequence = ClickSequence() }
    func click() {
        do { try refreshContext() } catch { lastError = String(describing: error); return }
        let time = ProcessInfo.processInfo.systemUptime, interval = Double(config?.click_interval_ms ?? 300) / 1000
        if sequence.count == 0 { sequencePreview = preview }
        if let expired = sequence.press(at: time, interval: interval) { dispatchClick(expired) }
        pendingClick?.cancel()
        if sequence.count >= 3 { dispatchClick(sequence.take()); return }
        let task = DispatchWorkItem { [weak self] in guard let self else { return }; self.dispatchClick(self.sequence.take()) }
        pendingClick = task; DispatchQueue.main.asyncAfter(deadline: .now() + interval, execute: task)
    }
    func dispatchClick(_ count: Int) {
        guard count > 0, let config else { return }
        let bindings = activeProfile?.bindings ?? config.bindings
        let action = count == 1 ? bindings.single : count == 2 ? bindings.double : bindings.triple
        record(gesture: count == 1 ? "single" : count == 2 ? "double" : "triple", action: action)
        if !sequencePreview && !preview && action != "none" { controller.perform(action) }
    }
    func turn(_ direction: Int, shift: Bool) {
        do {
            try refreshContext()
            guard let config else { return }
            let bindings = activeProfile?.bindings ?? config.bindings
            let action = shift ? bindings.shift_rotation : bindings.rotation
            record(gesture: "\(shift ? "shift_" : "")\(direction > 0 ? "right" : "left")", action: action)
            guard !preview else { return }
            switch action {
            case "volume": try changeOutputVolume(direction > 0 ? 0.0625 : -0.0625)
            case "seek": controller.perform(direction > 0 ? "seek_forward" : "seek_backward")
            case "zoom", "tabs", "scroll": try performContextTurn(action, direction: direction, expectedBundle: activeBundle)
            default: break
            }
        } catch { lastError = String(describing: error) }
    }
    func record(gesture: String, action: String) {
        eventID += 1
        events.insert(GestureEvent(id: eventID, gesture: gesture, action: action, profile: activeProfile?.name ?? "Default", preview: preview), at: 0)
        events = Array(events.prefix(8))
    }
    func snapshot() -> [String: Any] {
        return ["supported": true, "youtube_supported": true, "accessibility": AXIsProcessTrusted() && CGPreflightPostEventAccess(),
            "hotkey_active": !hotkeys.isEmpty, "music_busy": controller.isBusy, "music_automation": musicAutomationPermission(config?.music_service ?? "youtube_music"), "devices": jsonValue(devices), "youtube_apps": jsonValue(apps),
            "youtube_app_id": youtubeBundle.isEmpty ? NSNull() : youtubeBundle as Any,
            "installed_apps": jsonValue(installed), "active_app": activeBundle as Any? ?? NSNull(),
            "active_profile": activeProfile?.id as Any? ?? NSNull(), "preview": preview,
            "events": jsonValue(events), "message": message, "error": lastError as Any? ?? NSNull()]
    }
    func shutdown() throws {
        controller.cancel(); cancelClicks(); pendingProfile?.cancel(); pendingProfile = nil; timer?.invalidate(); timer = nil
        if let applicationObserver { NSWorkspace.shared.notificationCenter.removeObserver(applicationObserver) }; applicationObserver = nil
        if let instanceObserver { DistributedNotificationCenter.default().removeObserver(instanceObserver) }; instanceObserver = nil
        if let backgroundActivity { ProcessInfo.processInfo.endActivity(backgroundActivity) }; backgroundActivity = nil
        defer { unregisterHotkeys() }
        if let directory { _ = try updateKnobMappings(target: nil, directory: directory) }
    }
    @objc func openWindow() { NSApp.activate(ignoringOtherApps: true); NSApp.windows.first { $0.title == "AK820 Pro Modder" }?.makeKeyAndOrderFront(nil) }
    @objc func quitApplication() { NSApp.terminate(nil) }
}
private let controls = NativeControls()
@_cdecl("ak820_mac_controls_request")
public func ak820MacControlsRequest(_ input: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>? {
    do {
        guard Thread.isMainThread else { throw MusicError.failed("Mac controls must run on the main thread") }
        guard let request = try JSONSerialization.jsonObject(with: Data(String(cString: input).utf8)) as? [String: Any] else { throw MusicError.failed("Invalid request") }
        switch request["operation"] as? String {
        case "configure": try controls.configure(request)
        case "refresh": controls.lastError = nil; controls.apps = youtubeApps(); controls.installed = installedApplications(); try controls.refresh(force: true)
        case "press", "liked", "action":
            guard controls.advanced else { throw MusicError.failed("Enable custom knob controls first") }
            controls.controller.perform(request["operation"] as? String == "action" ? request["action"] as? String ?? "toggle" : request["operation"] as? String == "liked" ? "liked" : "toggle")
        case "music_access": controls.controller.requestMusicAccess()
        case "automation_settings": NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation")!)
        case "preview":
            guard controls.advanced else { throw MusicError.failed("Save custom knob controls before testing") }
            controls.cancelClicks(); controls.previewUntil = request["enabled"] as? Bool == true ? ProcessInfo.processInfo.systemUptime + 30 : 0; controls.events = []
        case "permission":
            NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")!)
        case "base_edit":
            guard NSWorkspace.shared.frontmostApplication?.bundleIdentifier == Bundle.main.bundleIdentifier else { throw MusicError.failed("Open Modder before editing base key mappings") }
            try controls.refreshContext(force: true)
        case "shutdown": try controls.shutdown()
        case "status": break
        default: throw MusicError.failed("Unknown Mac control operation")
        }
        return try encodeJSON(controls.snapshot()).withCString { strdup($0) }
    } catch {
        if Thread.isMainThread { controls.lastError = String(describing: error) }
        return ((try? encodeJSON(["error": String(describing: error)])) ?? "{}").withCString { strdup($0) }
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
        let oldData = try JSONEncoder().encode(["bluetooth-1": MuteOwnership(original: playUsage, applied: triggerUsage)])
        let migrated = try decodeOwnership(oldData)
        guard migrated.version == 2, migrated.profiles["bluetooth-1"]?[String(muteUsage)]?.original == playUsage else { return 7 }
        let targets = [muteUsage: triggerUsage, volumeUpUsage: turnUpUsage, volumeDownUsage: turnDownUsage, UInt64((7 << 32) | 6): UInt64((7 << 32) | 7)]
        let (all, owners) = try plannedRules([key, custom], owners: [:], targets: targets)
        let (againAll, againOwners) = try plannedRules(all, owners: owners, targets: targets)
        guard all == againAll, owners == againOwners else { return 8 }
        let (undoAll, undoOwners) = try plannedRules(all, owners: owners, targets: [:])
        guard undoOwners.isEmpty, Set(undoAll.compactMap { $0[mappingSource] }) == Set([key[mappingSource]!, muteUsage]), undoAll.first(where: { $0[mappingSource] == muteUsage })?[mappingDestination] == playUsage else { return 9 }
        let changedExternally = all.filter { $0[mappingSource] != volumeUpUsage } + [[mappingSource: volumeUpUsage, mappingDestination: playUsage]]
        do { _ = try plannedRules(changedExternally, owners: owners, targets: targets); return 10 } catch { }
        let (externalUndo, _) = try plannedRules(changedExternally, owners: owners, targets: [:])
        guard externalUndo.contains([mappingSource: volumeUpUsage, mappingDestination: playUsage]), !externalUndo.contains(where: { $0[mappingDestination] == turnDownUsage }) else { return 11 }
        let lost = try plannedRules([], owners: owners, targets: targets)
        guard lost.1[String(muteUsage)]?.original == playUsage else { return 12 }
        var clicks = ClickSequence()
        guard clicks.press(at: 1, interval: 0.3) == nil, clicks.take() == 1,
              clicks.press(at: 2, interval: 0.3) == nil, clicks.press(at: 2.1, interval: 0.3) == nil, clicks.take() == 2,
              clicks.press(at: 3, interval: 0.3) == nil, clicks.press(at: 3.1, interval: 0.3) == nil, clicks.press(at: 3.2, interval: 0.3) == nil, clicks.take() == 3 else { return 13 }
        _ = clicks.press(at: 4, interval: 0.3)
        guard clicks.press(at: 4.5, interval: 0.3) == 1, clicks.take() == 1 else { return 14 }
        guard chooseMusicService(preferred: "spotify", follow: false, playing: ["apple_music"], available: ["apple_music"], last: "apple_music") == "spotify",
              chooseMusicService(preferred: "youtube_music", follow: true, playing: ["spotify"], available: ["spotify"], last: nil) == "spotify",
              chooseMusicService(preferred: "youtube_music", follow: true, playing: [], available: ["spotify"], last: "spotify") == "spotify",
              chooseMusicService(preferred: "youtube_music", follow: true, playing: [], available: [], last: "apple_music") == "youtube_music" else { return 15 }
        guard secondsFromClock("1:02:03") == 3723, secondsFromClock("3:15") == 195, secondsFromClock("1:99") == nil, secondsFromClock("50%") == nil else { return 16 }
        let profile = AppProfile(id: "browser", name: "Browser", bundle_id: "org.example.browser", enabled: true, bindings: GestureBindings(), remaps: [ProfileRemap(source: 4, destination: 5)])
        var config = ControlConfig(knob_mode: "smart", launch_at_login: true, youtube_app_id: nil, music_service: "spotify", follow_playing: false, capture_rotation: true, click_interval_ms: 300, bindings: GestureBindings(), profiles: [profile])
        try config.validate()
        guard config.profile(for: "org.example.browser")?.id == "browser", config.profile(for: "org.example.other") == nil else { return 17 }
        config.profiles.append(profile)
        do { try config.validate(); return 18 } catch { }
        config.profiles = []; config.bindings.single = "shell"
        do { try config.validate(); return 19 } catch { }
        guard timelineStep(value: 25, maximum: 100, description: "1:00 of 4:00", delta: 5) == 5.0 / 2.4,
              timelineStep(value: 60, maximum: 240, description: "1:00", delta: 5) == 5,
              timelineStep(value: 0, maximum: 100, description: "0:00 of 4:00", delta: 5) == 5.0 / 2.4,
              timelineStep(value: 50, maximum: 100, description: "−2:00 remaining", delta: 5) == nil,
              timelineStep(value: 50, maximum: 100, description: nil, delta: 5) == nil else { return 20 }
        guard durationSeconds("1 Minutes 5 Seconds") == 65,
              durationSeconds("1 минута 5 секунд") == 65,
              timelineStep(value: 65, maximum: 241, description: "1 Minutes 5 Seconds of 4 Minutes 1 Seconds", delta: 5) == 5,
              timelineStep(value: 0, maximum: 241, description: "0 Seconds of 4 Minutes 1 Seconds", delta: -5) == -5,
              timelineStep(value: 25, maximum: 100, description: "1:00 / 4:00", delta: 5) == 5.0 / 2.4 else { return 21 }
        let specifier = musicProperty("pPos")
        guard specifier.descriptorType == typeObjectSpecifier,
              specifier.forKeyword(AEKeyword(keyAEKeyData))?.typeCodeValue == fourCC("pPos"),
              nativeMusicBundle("apple_music") == "com.apple.Music",
              nativeMusicBundle("youtube_music") == nil,
              nativeMusicBundle("arbitrary.bundle") == nil else { return 22 }
        let lockDirectory = FileManager.default.temporaryDirectory.appendingPathComponent("ak820-lock-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: lockDirectory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: lockDirectory) }
        let lockFile = lockDirectory.appendingPathComponent("test.lock")
        do {
            let lease = try NativeFileLock(lockFile)
            try withExtendedLifetime(lease) {
                guard fcntl(lease.descriptor, F_GETFD) & FD_CLOEXEC != 0 else { throw MusicError.failed("Lock leaks into child processes") }
                do { _ = try NativeFileLock(lockFile); throw MusicError.failed("Reentrant lock was accepted") }
                catch NativeLockError.busy { }
            }
        }
        let released = try NativeFileLock(lockFile)
        withExtendedLifetime(released) { }
        return 0
    } catch { return 6 }
}
