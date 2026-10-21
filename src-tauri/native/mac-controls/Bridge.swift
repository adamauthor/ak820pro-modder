// All public entry points run on Tauri's AppKit thread. Hardware mappings and
// status-item ownership stay in this process, including after the window closes.
import AppKit
import ApplicationServices
import Carbon
import Darwin
import Foundation

func jsonValue<T: Encodable>(_ value: T) -> Any { (try? JSONSerialization.jsonObject(with: JSONEncoder().encode(value))) ?? NSNull() }
final class NativeControls: NSObject, NSMenuDelegate {
    var config: ControlConfig?
    var directory: URL?
    var apps: [YouTubeApp] = []
    var installed: [InstalledApp] = []
    var devices: [ConnectedKeyboard] = []
    var message = "Ready"
    var lastError: String? {
        // Errors also go to Modder's log file, where they can be diagnosed.
        didSet { if let lastError, lastError != oldValue { emit(["event": "log", "message": lastError]) } }
    }
    var timer: Timer?
    var hotkeys: [EventHotKeyRef] = []
    var handler: EventHandlerRef?
    var registeredRotation = false
    var heldClick = false
    let controller = MusicController()
    let detectors = Detectors()
    let battery = BluetoothBattery()
    var statusItem: NSStatusItem?
    var menuState: [String: Any] = [:]
    var backgroundActivity: NSObjectProtocol?
    var applicationObserver: NSObjectProtocol?
    var instanceObserver: NSObjectProtocol?
    // hidutil runs on this queue so AppKit never waits on child processes.
    let mappingQueue = DispatchQueue(label: "local.ak820pro.mappings")
    var mappingGeneration = 0
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
        installStatusItem()
        let previous = config, previousYouTube = youtubeBundle
        cancelClicks(); controller.cancel(); previewUntil = 0
        config = next; directory = URL(fileURLWithPath: path, isDirectory: true)
        apps = youtubeApps(); installed = installedApplications()
        youtubeBundle = (apps.first { $0.id == next.youtube_app_id } ?? (next.youtube_app_id == nil ? apps.first : nil))?.id ?? ""
        controller.service = next.music_service; controller.followPlaying = next.follow_playing
        do { try refresh(force: true, wait: true); lastError = nil }
        catch {
            config = previous; youtubeBundle = previousYouTube
            controller.service = previous?.music_service ?? "youtube_music"; controller.followPlaying = previous?.follow_playing ?? true
            try? refresh(force: true, wait: true); throw error
        }
        if applicationObserver == nil {
            applicationObserver = NSWorkspace.shared.notificationCenter.addObserver(forName: NSWorkspace.didActivateApplicationNotification, object: nil, queue: .main) { [weak self] note in
                let bundle = (note.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication)?.bundleIdentifier
                emit(["event": "frontmost", "bundle": bundle as Any? ?? NSNull()])
                self?.refreshQuietly()
            }
        }
        if instanceObserver == nil {
            instanceObserver = DistributedNotificationCenter.default().addObserver(forName: showSettingsNotification, object: nil, queue: .main) { [weak self] _ in self?.openWindow() }
        }
        if timer == nil {
            let timer = Timer(timeInterval: 10, repeats: true) { [weak self] _ in
                guard let self else { return }
                do { try self.refresh(force: true, wait: false) } catch { self.report(error) }
            }
            self.timer = timer; RunLoop.main.add(timer, forMode: .common)
        }
    }
    func report(_ error: Error) {
        if String(describing: error) != "Release held keys to switch profile mappings" { lastError = String(describing: error) }
    }
    func refreshQuietly() { do { try refreshContext(wait: false) } catch { report(error) } }
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
    func refresh(force: Bool = false, wait: Bool) throws {
        if advanced { try registerHotkeys(rotation: config?.capture_rotation == true) }
        try refreshContext(force: force, wait: wait)
        if !advanced { unregisterHotkeys() }
        updateMessage()
    }
    func updateMessage() {
        if advanced && !AXIsProcessTrusted() { message = "Enable Accessibility for AK820 Pro Modder, then Stop controls and quit and reopen it" }
        else if devices.isEmpty { message = "Waiting for AK820 over USB or Bluetooth" }
        else if mode == "original" { message = "Previous knob mapping restored" }
        else if message == "Ready" || message.hasPrefix("Waiting") || message.hasPrefix("Previous") { message = "Ready — USB and Bluetooth" }
    }
    /// Applies the mappings for the frontmost app. wait=true is for user
    /// actions that must report errors; background refreshes never block AppKit.
    func refreshContext(force: Bool = false, wait: Bool) throws {
        let bundle = NSWorkspace.shared.frontmostApplication?.bundleIdentifier
        let profile = advanced ? config?.profile(for: bundle) : nil
        guard force || bundle != activeBundle || profile != activeProfile else { return }
        if bundle != activeBundle || profile != activeProfile { cancelClicks() }
        if (profile?.remaps ?? []) != (activeProfile?.remaps ?? []),
           (0..<128).contains(where: { ![kVK_F18, kVK_F19, kVK_F20].contains($0) && CGEventSource.keyState(.combinedSessionState, key: CGKeyCode($0)) }) {
            pendingProfile?.cancel()
            let task = DispatchWorkItem { [weak self] in self?.refreshQuietly() }
            pendingProfile = task; DispatchQueue.main.asyncAfter(deadline: .now() + 0.08, execute: task)
            throw MusicError.failed("Release held keys to switch profile mappings")
        }
        pendingProfile?.cancel(); pendingProfile = nil
        guard let directory else { return }
        let target = mode == "original" ? nil : advanced ? triggerUsage : playUsage
        let rotation = advanced && config?.capture_rotation == true
        let remaps = profile?.remaps ?? [], base = config?.base_remaps ?? []
        if bundle != activeBundle || profile != activeProfile { emit(["event": "profile", "name": profile?.name as Any? ?? NSNull()]) }
        activeBundle = bundle; activeProfile = profile
        mappingGeneration += 1
        let generation = mappingGeneration
        let work = { try updateControlMappings(target: target, rotation: rotation, remaps: remaps, base: base, directory: directory) }
        if wait {
            devices = try mappingQueue.sync(execute: work)
            if lastError == "Release held keys to switch profile mappings" { lastError = nil }
            return
        }
        mappingQueue.async { [weak self] in
            let result = Result(catching: work)
            DispatchQueue.main.async {
                guard let self, generation == self.mappingGeneration else { return }
                switch result {
                case .success(let devices):
                    let changed = devices.map(\.profile) != self.devices.map(\.profile)
                    self.devices = devices; self.updateMessage()
                    if changed { emit(["event": "devices", "devices": jsonValue(devices)]) }
                case .failure(let error): self.report(error)
                }
            }
        }
    }
    func cancelClicks() { pendingClick?.cancel(); pendingClick = nil; sequence = ClickSequence() }
    func click() {
        refreshQuietly()
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
        guard !sequencePreview && !preview && action != "none" else { return }
        perform(click: action)
    }
    func perform(click action: String) {
        do {
            switch action {
            case "mute": try toggleOutputMute(); message = "Sound mute toggled"
            case "mic_mute": message = try toggleMicrophoneMute() ? "Microphone muted" : "Microphone on"
            case "pomodoro": emit(["event": "knob_action", "action": "pomodoro"])
            default: controller.perform(action)
            }
        } catch { lastError = String(describing: error) }
    }
    func turn(_ direction: Int, shift: Bool) {
        refreshQuietly()
        guard let config else { return }
        let bindings = activeProfile?.bindings ?? config.bindings
        let action = shift ? bindings.shift_rotation : bindings.rotation
        record(gesture: "\(shift ? "shift_" : "")\(direction > 0 ? "right" : "left")", action: action)
        guard !preview else { return }
        do {
            switch action {
            case "volume": try stepVolume(direction)
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
        detectors.configure(teams: false, telegram: false)
        if let applicationObserver { NSWorkspace.shared.notificationCenter.removeObserver(applicationObserver) }; applicationObserver = nil
        if let instanceObserver { DistributedNotificationCenter.default().removeObserver(instanceObserver) }; instanceObserver = nil
        if let backgroundActivity { ProcessInfo.processInfo.endActivity(backgroundActivity) }; backgroundActivity = nil
        defer { unregisterHotkeys() }
        if let directory { _ = try mappingQueue.sync { try updateKnobMappings(target: nil, directory: directory) } }
    }

    // MARK: Menu bar

    func installStatusItem() {
        guard statusItem == nil else { return }
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        // Keep the item narrow: a long title is hidden behind the notch.
        item.button?.image = NSImage(systemSymbolName: "keyboard", accessibilityDescription: "AK820 Pro Modder")
        item.button?.image?.isTemplate = true
        item.button?.imagePosition = .imageLeading
        item.button?.toolTip = "AK820 Pro Modder"
        item.autosaveName = "AK820ProModder"
        let menu = NSMenu(); menu.delegate = self; menu.autoenablesItems = false
        item.menu = menu; statusItem = item
        updateStatusTitle()
    }
    func setMenuState(_ state: [String: Any]) { menuState = state; updateStatusTitle() }
    func updateStatusTitle() {
        // Only the Pomodoro countdown and a missing permission are shown next
        // to the icon; battery and everything else live in the menu.
        var title = ""
        if let timer = menuState["pomodoro_label"] as? String, !timer.isEmpty { title = " \(timer)" }
        if !(AXIsProcessTrusted() && CGPreflightPostEventAccess()) { title += " ⚠︎" }
        statusItem?.button?.title = title
    }
    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        func item(_ title: String, _ event: [String: Any]? = nil, checked: Bool = false, enabled: Bool = true) -> NSMenuItem {
            let entry = NSMenuItem(title: title, action: event == nil ? nil : #selector(menuEvent(_:)), keyEquivalent: "")
            entry.target = self; entry.representedObject = event; entry.state = checked ? .on : .off
            entry.isEnabled = enabled && (event != nil); return entry
        }
        func submenu(_ title: String, _ items: [NSMenuItem]) -> NSMenuItem {
            let parent = NSMenuItem(title: title, action: nil, keyEquivalent: ""), child = NSMenu()
            child.autoenablesItems = false; items.forEach(child.addItem); parent.submenu = child; return parent
        }
        let ru = (menuState["language"] as? String) == "ru"
        func tr(_ en: String, _ russian: String) -> String { ru ? russian : en }
        let open = NSMenuItem(title: tr("Open AK820 Pro Modder", "Открыть AK820 Pro Modder"), action: #selector(openWindow), keyEquivalent: "")
        open.target = self; menu.addItem(open)
        if !(AXIsProcessTrusted() && CGPreflightPostEventAccess()) {
            let fix = NSMenuItem(title: tr("⚠︎ Allow Accessibility (knob clicks, Telegram, players)…", "⚠︎ Разрешите Универсальный доступ (клики, Telegram, плееры)…"), action: #selector(openAccessibility), keyEquivalent: "")
            fix.target = self; menu.addItem(fix)
        }
        if let level = menuState["battery"] as? Int { menu.addItem(item(tr("Battery: \(level)%", "Батарея: \(level)%"), enabled: false)) }
        if let profile = activeProfile?.name { menu.addItem(item(tr("Profile: \(profile)", "Профиль: \(profile)"), enabled: false)) }
        if let status = menuState["pomodoro_status"] as? String, !status.isEmpty { menu.addItem(item(status, enabled: false)) }
        menu.addItem(.separator())
        let modes = [("original", tr("Original action", "Исходное действие")), ("play_pause", tr("Play / pause", "Воспроизведение / пауза")),
                     ("youtube_music", "YouTube Music"), ("smart", tr("Custom gestures", "Свои жесты"))]
        menu.addItem(submenu(tr("Knob", "Колёсико"), modes.map { item($0.1, ["event": "menu", "command": "knob_mode", "value": $0.0], checked: mode == $0.0) }))
        let usb = menuState["usb"] as? Bool == true
        let lighting = [("off", tr("Off", "Выключить")), ("white", tr("White", "Белый")), ("warm", tr("Warm", "Тёплый")), ("rainbow", tr("Rainbow", "Радуга")),
                        ("breath", tr("Breathing", "Дыхание")), ("restore", tr("My lighting", "Моя подсветка"))]
        menu.addItem(submenu(tr("Lighting", "Подсветка") + (usb ? "" : tr(" (USB)", " (USB)")), lighting.map { item($0.1, ["event": "menu", "command": "lighting", "value": $0.0], enabled: usb) }))
        let smart = menuState["smart"] as? [String: Bool] ?? [:]
        menu.addItem(submenu(tr("Smart lighting", "Умная подсветка"), [
            item(tr("Teams calls", "Звонки Teams"), ["event": "menu", "command": "smart", "value": "teams"], checked: smart["teams"] == true),
            item(tr("Telegram messages", "Сообщения Telegram"), ["event": "menu", "command": "smart", "value": "telegram"], checked: smart["telegram"] == true),
            item(tr("App lighting", "Подсветка по приложениям"), ["event": "menu", "command": "smart", "value": "apps"], checked: smart["apps"] == true),
        ]))
        let phase = menuState["pomodoro"] as? String ?? "idle"
        var pomodoro = [item(tr("Start focus", "Начать фокус"), ["event": "menu", "command": "pomodoro", "value": "start"])]
        if phase != "idle" {
            pomodoro.append(item(phase == "paused" ? tr("Resume", "Продолжить") : tr("Pause", "Пауза"), ["event": "menu", "command": "pomodoro", "value": phase == "paused" ? "resume" : "pause"]))
            pomodoro.append(item(tr("Skip phase", "Пропустить этап"), ["event": "menu", "command": "pomodoro", "value": "skip"]))
            pomodoro.append(item(tr("Stop", "Остановить"), ["event": "menu", "command": "pomodoro", "value": "stop"]))
        }
        menu.addItem(submenu(tr("Pomodoro", "Помодоро"), pomodoro))
        let scenes = [("daily", tr("Greeting card", "Открытка дня")), ("weather", tr("Weather", "Погода")), ("now_playing", tr("Now playing", "Сейчас играет"))]
        menu.addItem(submenu(tr("Anime screen", "Аниме-экран") + (usb ? "" : " (USB)"), scenes.map { item($0.1, ["event": "menu", "command": "anime", "value": $0.0], enabled: usb) }))
        menu.addItem(.separator())
        let quit = NSMenuItem(title: tr("Stop controls and quit", "Остановить и выйти"), action: #selector(quitApplication), keyEquivalent: "")
        quit.target = self; menu.addItem(quit)
    }
    @objc func openAccessibility() {
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        _ = AXIsProcessTrustedWithOptions(options)
        NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")!)
    }
    @objc func menuEvent(_ sender: NSMenuItem) { if let event = sender.representedObject as? [String: Any] { emit(event) } }
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
        case "refresh": controls.lastError = nil; controls.apps = youtubeApps(); controls.installed = installedApplications(); try controls.refresh(force: true, wait: true)
        case "press", "liked", "action":
            guard controls.advanced else { throw MusicError.failed("Enable custom knob controls first") }
            controls.perform(click: request["operation"] as? String == "action" ? request["action"] as? String ?? "toggle" : request["operation"] as? String == "liked" ? "liked" : "toggle")
        case "music_access": controls.controller.requestMusicAccess()
        case "automation_settings": NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation")!)
        case "preview":
            guard controls.advanced else { throw MusicError.failed("Save custom knob controls before testing") }
            controls.cancelClicks(); controls.previewUntil = request["enabled"] as? Bool == true ? ProcessInfo.processInfo.systemUptime + 30 : 0; controls.events = []
        case "permission":
            NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")!)
        case "menu_state": controls.setMenuState(request["state"] as? [String: Any] ?? [:])
        case "notify": postNotification(title: request["title"] as? String ?? "AK820", body: request["body"] as? String ?? "")
        case "detectors": controls.detectors.configure(teams: request["teams"] as? Bool == true, telegram: request["telegram"] as? Bool == true)
        case "now_playing":
            // Read on the AppKit thread; AX calls are bounded by short timeouts.
            var result = controls.snapshot()
            result["now_playing"] = youtubeNowPlaying() ?? NSNull()
            return try encodeJSON(result).withCString { strdup($0) }
        case "telegram_test": emit(["event": "telegram_message", "unread": -1])
        case "battery": controls.battery.read { level in emit(["event": "bluetooth_battery", "level": level as Any? ?? NSNull()]) }
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
        guard youtubeActionAfterReopen("toggle", alreadyRunning: true) == "toggle",
              youtubeActionAfterReopen("toggle", alreadyRunning: false) == "play",
              youtubeActionAfterReopen("seek_forward", alreadyRunning: true) == "seek_forward",
              youtubeActionAfterReopen("next", alreadyRunning: true) == "next" else { return 23 }
        let layered = layeredRemaps(base: [ProfileRemap(source: 4, destination: 5), ProfileRemap(source: 6, destination: 7)], profile: [ProfileRemap(source: 6, destination: 8)])
        guard layered == [ProfileRemap(source: 4, destination: 5), ProfileRemap(source: 6, destination: 8)] else { return 24 }
        do { _ = try decodeOwnership(Data(#"{"version":3,"profiles":{}}"#.utf8)); return 25 } catch { }
        guard badgeCount(nil) == 0, badgeCount("") == 0, badgeCount("12") == 12, badgeCount("•") == 1,
              isTeamsBundle("com.microsoft.teams2"), !isTeamsBundle("com.apple.Music") else { return 26 }
        var muted = config
        muted.bindings.single = "mute"; muted.bindings.double = "mic_mute"; muted.bindings.triple = "pomodoro"
        muted.base_remaps = [ProfileRemap(source: 4, destination: 111)]
        do { try muted.validate(); return 27 } catch { }
        muted.base_remaps = [ProfileRemap(source: 4, destination: 5)]
        try muted.validate()
        guard GestureBindings().single == "toggle" else { return 28 } // mute is never a default
        guard parseStatusLabel(#""StatusLabel"={ "label"="5" }"#) == "5", parseStatusLabel(#""StatusLabel"={ "label"=kCFNULL }"#) == nil,
              parseStatusLabel("") == nil else { return 29 }
        return 0
    } catch { return 6 }
}
