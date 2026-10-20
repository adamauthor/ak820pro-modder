import AppKit
import ApplicationServices
import Foundation

var youtubeBundle = ""

func axAttribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}
func axChildren(_ element: AXUIElement) -> [AXUIElement] { axAttribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? [] }
func axName(_ element: AXUIElement) -> String {
    for attribute in [kAXTitleAttribute, kAXDescriptionAttribute, kAXHelpAttribute] {
        if let value = axAttribute(element, attribute) as? String, !value.isEmpty { return value }
    }
    return ""
}
func axRole(_ element: AXUIElement) -> String { axAttribute(element, kAXRoleAttribute) as? String ?? "" }

// Reverse traversal finds the player toolbar before the long home-page feed.
func axFind(_ root: AXUIElement, limit: Int = 1800, seconds: TimeInterval = 4, reverse: Bool = true, where predicate: (AXUIElement) -> Bool) -> AXUIElement? {
    var stack = [root]
    var count = 0
    let deadline = Date().addingTimeInterval(seconds)
    while let element = stack.popLast(), count < limit, Date() < deadline {
        count += 1
        if predicate(element) { return element }
        let children = axChildren(element)
        if reverse { stack.append(contentsOf: children) }
        else { stack.append(contentsOf: children.reversed()) }
    }
    return nil
}

struct YouTubePlayer {
    let button: AXUIElement
    let playing: Bool
}
func playerRoot(_ pid: pid_t) -> AXUIElement {
    let root = AXUIElementCreateApplication(pid)
    AXUIElementSetMessagingTimeout(root, 0.5)
    if let windows = axAttribute(root, kAXWindowsAttribute) as? [AXUIElement],
       let window = windows.first(where: { axName($0).contains("YouTube Music") }) ?? windows.first {
        AXUIElementSetMessagingTimeout(window, 0.5)
        return window
    }
    return root
}
func findPlayer(_ root: AXUIElement) -> YouTubePlayer? {
    guard let toolbar = axFind(root, where: { element in
        axRole(element) == kAXToolbarRole && ["player bar", "панель проигрывателя", "панель плеера"].contains(axName(element).lowercased())
    }) else { return nil }
    guard let button = axFind(toolbar, limit: 60, seconds: 1, where: { element in
        axRole(element) == kAXButtonRole && ["play", "pause", "воспроизвести", "пауза"].contains(axName(element).lowercased())
    }) else { return nil }
    if (axAttribute(button, kAXEnabledAttribute) as? NSNumber)?.boolValue == false { return nil }
    return YouTubePlayer(button: button, playing: ["pause", "пауза"].contains(axName(button).lowercased()))
}
func pressAX(_ element: AXUIElement) throws {
    guard AXUIElementPerformAction(element, kAXPressAction as CFString) == .success else { throw MusicError.failed("YT Music did not accept the player action") }
}
func startLikedMusic(_ root: AXUIElement, canPress: () -> Bool = { true }) throws {
    guard let web = axFind(root, limit: 100, seconds: 2, where: { axRole($0) == "AXWebArea" }) else {
        throw MusicError.failed("YT Music page is not ready")
    }
    var playlist: AXUIElement?
    // The pinned sidebar precedes the large home feed in the web area's children.
    for section in axChildren(web).prefix(2) {
        playlist = axFind(section, limit: 250, seconds: 2, reverse: false, where: { element in
        let title = axName(element).lowercased()
            return axRole(element) == "AXLink" && (title.hasPrefix("liked music") || title.hasPrefix("мне понравилось"))
        })
        if playlist != nil { break }
    }
    guard let playlist else {
        if let header = axChildren(web).first,
           let guide = axFind(header, limit: 80, seconds: 1, reverse: false, where: { ["guide", "меню"].contains(axName($0).lowercased()) }),
           (axAttribute(guide, kAXValueAttribute) as? NSNumber)?.boolValue == false, canPress() {
            try pressAX(guide)
        }
        throw MusicError.failed("Liked Music sidebar is not ready")
    }
    guard let play = axFind(playlist, limit: 40, seconds: 1, where: { element in
        axRole(element) == kAXButtonRole && ["play", "воспроизвести"].contains(axName(element).lowercased())
    }) else { throw MusicError.failed("Liked Music play button is unavailable") }
    guard canPress() else { throw MusicError.failed("Playback action cancelled") }
    try pressAX(play)
}

func standardMediaKey() throws {
    // This is the macOS Play/Pause media event, not a typing key or an Apple event.
    for down in [true, false] {
        let state = down ? 0x0a : 0x0b
        guard let event = NSEvent.otherEvent(with: .systemDefined, location: .zero, modifierFlags: NSEvent.ModifierFlags(rawValue: UInt(state << 8)), timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: 0, context: nil, subtype: 8, data1: (16 << 16) | (state << 8), data2: -1)?.cgEvent else { throw MusicError.failed("Cannot create Play/Pause event") }
        event.post(tap: .cghidEventTap)
    }
}

final class MusicController {
    private let worker = DispatchQueue(label: "local.ak820pro.music-actions")
    private var busy = false // Main queue only, prevents duplicate starts on repeated clicks.
    private let requestLock = NSLock()
    private var requestID: UInt64 = 0
    private func beginRequest() -> UInt64 {
        requestLock.lock(); defer { requestLock.unlock() }
        requestID &+= 1
        return requestID
    }
    private func isCurrent(_ request: UInt64) -> Bool {
        requestLock.lock(); defer { requestLock.unlock() }
        return requestID == request
    }
    func cancel() { _ = beginRequest(); busy = false }
    var status: (String) -> Void = { _ in }

    func handleClick() {
        guard !busy else { return }
        guard AXIsProcessTrusted() else { status("Enable Accessibility for AK820 Pro Modder"); return }
        busy = true
        let request = beginRequest()
        let bundle = youtubeBundle
        worker.async { [self] in
            guard isCurrent(request) else { return }
            let audioActive = try? outputIsActive()
            let app = NSRunningApplication.runningApplications(withBundleIdentifier: bundle).first
            let current = app.flatMap { findPlayer(playerRoot($0.processIdentifier)) }
            do {
                guard isCurrent(request) else { return }
                switch musicAction(audioActive: audioActive, youtubePlaying: current?.playing == true) {
                case .pauseYouTube:
                    try pressAX(current!.button)
                    finish("Paused", request: request)
                case .standardMediaKey:
                    try standardMediaKey()
                    finish("Active player: Play/Pause", request: request)
                case .startYouTube:
                    if let current {
                        try pressAX(current.button)
                        try confirmPlayback(playerRoot(app!.processIdentifier), request: request)
                        finish("YouTube Music: playing", request: request)
                    } else { launchAndPlay(bundle: bundle, request: request) }
                }
            } catch { finish("Error: \(error)", request: request) }
        }
    }

    func playLikedMusic() {
        guard !busy, AXIsProcessTrusted() else { return }
        busy = true
        launchAndPlay(forceLiked: true, bundle: youtubeBundle, request: beginRequest())
    }

    private func launchAndPlay(forceLiked: Bool = false, bundle: String, request: UInt64) {
        DispatchQueue.main.async { [self] in
            guard isCurrent(request) else { return }
            status("Opening YouTube Music…")
            guard let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundle) else { finish("Install YouTube Music as a Safari web app first", request: request); return }
            let configuration = NSWorkspace.OpenConfiguration()
            configuration.activates = true
            let ready: (NSRunningApplication?, Error?) -> Void = { [self] application, error in
                guard isCurrent(request) else { return }
                guard let application, error == nil else { finish("Could not open YouTube Music", request: request); return }
                worker.async { [self] in
                    do {
                        let deadline = ProcessInfo.processInfo.systemUptime + 60
                        while ProcessInfo.processInfo.systemUptime < deadline {
                            guard isCurrent(request) else { return }
                            // Safari web apps can replace their initial window while loading.
                            // Resolve both the process and the AX window again on each attempt.
                            let running = NSRunningApplication.runningApplications(withBundleIdentifier: bundle).first ?? application
                            let root = playerRoot(running.processIdentifier)
                            if !forceLiked, let player = findPlayer(root) {
                                if !player.playing { guard isCurrent(request) else { return }; try pressAX(player.button) }
                                try confirmPlayback(root, request: request)
                                finish("YouTube Music: playing", request: request)
                                return
                            }
                            // The pinned Liked Music play action starts the playlist from an empty queue.
                            if (try? startLikedMusic(root, canPress: { isCurrent(request) })) != nil {
                                try confirmPlayback(root, request: request)
                                finish("YouTube Music: Liked Music", request: request)
                                return
                            }
                            Thread.sleep(forTimeInterval: 0.75)
                        }
                        throw MusicError.failed("YT Music is not ready or needs sign-in")
                    } catch { finish("Error: \(error)", request: request) }
                }
            }
            if let application = NSRunningApplication.runningApplications(withBundleIdentifier: bundle).first {
                application.activate(options: [.activateAllWindows])
                let root = AXUIElementCreateApplication(application.processIdentifier)
                if let windows = axAttribute(root, kAXWindowsAttribute) as? [AXUIElement], !windows.isEmpty { ready(application, nil); return }
            }
            NSWorkspace.shared.openApplication(at: url, configuration: configuration, completionHandler: ready)
        }
    }

    private func confirmPlayback(_ root: AXUIElement, request: UInt64) throws {
        for _ in 0..<12 {
            guard isCurrent(request) else { throw MusicError.failed("Playback action cancelled") }
            if findPlayer(root)?.playing == true { return }
            Thread.sleep(forTimeInterval: 0.25)
        }
        throw MusicError.failed("YT Music did not confirm playback; check the network or sign-in")
    }

    private func finish(_ message: String, request: UInt64) {
        DispatchQueue.main.async { [self] in guard isCurrent(request) else { return }; status(message); busy = false }
    }
}
