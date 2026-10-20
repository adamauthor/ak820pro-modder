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
func playerWindow(_ pid: pid_t) -> AXUIElement? {
    let root = AXUIElementCreateApplication(pid)
    AXUIElementSetMessagingTimeout(root, 0.5)
    if let windows = axAttribute(root, kAXWindowsAttribute) as? [AXUIElement],
       let window = windows.first(where: { axName($0).contains("YouTube Music") }) ?? windows.first {
        AXUIElementSetMessagingTimeout(window, 0.5)
        return window
    }
    return nil
}
func playerRoot(_ pid: pid_t) -> AXUIElement {
    playerWindow(pid) ?? AXUIElementCreateApplication(pid)
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
    guard AXUIElementPerformAction(element, kAXPressAction as CFString) == .success else { throw MusicError.failed("The music app did not accept the player action") }
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

struct MediaSurface {
    let root: AXUIElement
    let toggle: AXUIElement
    let playing: Bool
    let next: AXUIElement?
    let previous: AXUIElement?
}
func enabledButton(_ element: AXUIElement) -> Bool {
    axRole(element) == kAXButtonRole && (axAttribute(element, kAXEnabledAttribute) as? NSNumber)?.boolValue != false
}
let playNames = ["play", "воспроизвести", "воспроизведение", "start playback"]
let pauseNames = ["pause", "пауза", "приостановить", "pause playback"]
let nextNames = ["next", "next song", "next track", "следующий", "следующий трек", "следующая песня", "далее"]
let previousNames = ["previous", "previous song", "previous track", "предыдущий", "предыдущий трек", "предыдущая песня", "назад"]
func mediaSurface(_ root: AXUIElement, service: String) -> MediaSurface? {
    if service == "youtube_music" {
        guard let player = findPlayer(root), let toolbar = axFind(root, where: { axRole($0) == kAXToolbarRole && ["player bar", "панель проигрывателя", "панель плеера"].contains(axName($0).lowercased()) }) else { return nil }
        return MediaSurface(root: toolbar, toggle: player.button, playing: player.playing,
            next: axFind(toolbar, limit: 100, seconds: 1, where: { enabledButton($0) && nextNames.contains(axName($0).lowercased()) }),
            previous: axFind(toolbar, limit: 100, seconds: 1, where: { enabledButton($0) && previousNames.contains(axName($0).lowercased()) }))
    }
    guard let button = axFind(root, limit: 1200, seconds: 2, where: { element in
        let name = axName(element).lowercased()
        guard enabledButton(element), (playNames + pauseNames).contains(name) else { return false }
        let identifier = axAttribute(element, kAXIdentifierAttribute) as? String ?? ""
        if service == "apple_music" && identifier.contains("playbackTransportControl") { return true }
        var parent = element
        // A transport has Previous and Next controls in the same small group.
        // This rejects Play buttons in album cards or a recommendation feed.
        for _ in 0..<3 {
            guard let value = axAttribute(parent, kAXParentAttribute), CFGetTypeID(value) == AXUIElementGetTypeID() else { return false }
            parent = unsafeBitCast(value, to: AXUIElement.self)
            guard ![kAXApplicationRole, kAXWindowRole, "AXWebArea"].contains(axRole(parent)) else { return false }
            let previous = axFind(parent, limit: 60, seconds: 0.3, where: { enabledButton($0) && previousNames.contains(axName($0).lowercased()) })
            let next = axFind(parent, limit: 60, seconds: 0.3, where: { enabledButton($0) && nextNames.contains(axName($0).lowercased()) })
            if previous != nil && next != nil { return true }
        }
        return false
    }) else { return nil }
    func transport(_ names: [String], _ suffix: String) -> AXUIElement? {
        axFind(root, limit: 700, seconds: 1, where: { enabledButton($0) && (names.contains(axName($0).lowercased()) || (axAttribute($0, kAXIdentifierAttribute) as? String ?? "").hasSuffix(suffix)) })
    }
    return MediaSurface(root: root, toggle: button, playing: pauseNames.contains(axName(button).lowercased()), next: transport(nextNames, ".trailingTransportControl"), previous: transport(previousNames, ".leadingTransportControl"))
}
func seekSurface(_ surface: MediaSurface, by delta: Double) throws {
    guard let slider = axFind(surface.root, limit: 200, seconds: 1, where: { element in
        let name = axName(element).lowercased(), id = (axAttribute(element, kAXIdentifierAttribute) as? String ?? "").lowercased()
        return axRole(element) == kAXSliderRole && ["progress", "seek", "playback", "elapsed", "воспроизведения", "позиция"].contains(where: { name.contains($0) || id.contains($0) })
    }), let value = (axAttribute(slider, kAXValueAttribute) as? NSNumber)?.doubleValue,
        let maximum = (axAttribute(slider, kAXMaxValueAttribute) as? NSNumber)?.doubleValue, maximum > 0 else { throw MusicError.failed("This player's seek control is unavailable") }
    var settable = DarwinBoolean(false)
    guard AXUIElementIsAttributeSettable(slider, kAXValueAttribute as CFString, &settable) == .success, settable.boolValue else { throw MusicError.failed("This player does not expose seeking to Accessibility") }
    // AXValueDescription provides the elapsed time when the value is normalized.
    // Never interpret a percentage as seconds without a confirmed elapsed time.
    guard let step = timelineStep(value: value, maximum: maximum,
        description: axAttribute(slider, kAXValueDescriptionAttribute) as? String, delta: delta) else { throw MusicError.failed("Cannot determine the seek timeline units for this player") }
    guard AXUIElementSetAttributeValue(slider, kAXValueAttribute as CFString, NSNumber(value: min(maximum, max(0, value + step)))) == .success else { throw MusicError.failed("The player did not accept seeking") }
}
func serviceBundle(_ service: String, youtube: String) -> String {
    service == "spotify" ? "com.spotify.client" : service == "apple_music" ? "com.apple.Music" : youtube
}

func chooseMusicService(preferred: String, follow: Bool, playing: Set<String>, available: Set<String>, last: String?) -> String {
    guard follow else { return preferred }
    if playing.contains(preferred) { return preferred }
    if let active = ["youtube_music", "spotify", "apple_music"].first(where: playing.contains) { return active }
    if let last, available.contains(last) { return last }
    return preferred
}

final class MusicController {
    private let worker = DispatchQueue(label: "local.ak820pro.music-actions")
    var service = "youtube_music"
    var followPlaying = true
    private var lastRoute: String? // Worker queue only.
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
    func cancel() { _ = beginRequest(); busy = false; worker.async { [self] in lastRoute = nil } }
    var status: (String) -> Void = { _ in }

    func perform(_ action: String) {
        guard !busy else { return }
        guard AXIsProcessTrusted() else { status("Enable Accessibility for AK820 Pro Modder"); return }
        busy = true
        let request = beginRequest(), preferred = service, follow = followPlaying, youtube = youtubeBundle
        worker.async { [self] in
            guard isCurrent(request) else { return }
            var route = preferred
            var surfaces: [String: MediaSurface] = [:]
            for service in follow ? [preferred] + ["youtube_music", "spotify", "apple_music"].filter({ $0 != preferred }) : [preferred] {
                let bundle = serviceBundle(service, youtube: youtube)
                guard !bundle.isEmpty, let app = NSRunningApplication.runningApplications(withBundleIdentifier: bundle).first else { continue }
                if let surface = mediaSurface(playerRoot(app.processIdentifier), service: service) { surfaces[service] = surface }
            }
            route = chooseMusicService(preferred: preferred, follow: follow,
                playing: Set(surfaces.filter { $0.value.playing }.map(\.key)), available: Set(surfaces.keys), last: lastRoute)
            if action == "liked" { route = "youtube_music" }
            let bundle = serviceBundle(route, youtube: youtube)
            do {
                guard isCurrent(request) else { return }
                if action == "open" { openNative(bundle: bundle, play: false, route: route, request: request); return }
                if action == "liked" { launchAndPlay(forceLiked: true, bundle: youtube, request: request); return }
                if let surface = surfaces[route] {
                    switch action {
                    case "toggle":
                        try pressAX(surface.toggle)
                        if route == "youtube_music" && !surface.playing,
                           let app = NSRunningApplication.runningApplications(withBundleIdentifier: bundle).first {
                            try confirmPlayback(playerRoot(app.processIdentifier), request: request)
                        }
                    case "next": guard let button = surface.next else { throw MusicError.failed("Next track is unavailable in this player") }; try pressAX(button)
                    case "previous": guard let button = surface.previous else { throw MusicError.failed("Previous track is unavailable in this player") }; try pressAX(button)
                    case "seek_forward": try seekSurface(surface, by: 5)
                    case "seek_backward": try seekSurface(surface, by: -5)
                    default: throw MusicError.failed("Unknown music action")
                    }
                    lastRoute = route
                    finish("\(route == "youtube_music" ? "YouTube Music" : route == "spotify" ? "Spotify" : "Apple Music"): \(action == "toggle" ? surface.playing ? "paused" : "playing" : action)", request: request)
                } else if action == "toggle" {
                    lastRoute = route
                    if route == "youtube_music" { launchAndPlay(bundle: bundle, request: request) }
                    else { openNative(bundle: bundle, play: true, route: route, request: request) }
                } else { throw MusicError.failed("Start a track in the selected player first") }
            } catch { finish("Error: \(error)", request: request) }
        }
    }
    private func openNative(bundle: String, play: Bool, route: String, request: UInt64) {
        DispatchQueue.main.async { [self] in
            guard isCurrent(request) else { return }
            guard !bundle.isEmpty, let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundle) else { finish("Error: Install the selected music app, then Refresh", request: request); return }
            let configuration = NSWorkspace.OpenConfiguration(); configuration.activates = true
            NSWorkspace.shared.openApplication(at: url, configuration: configuration) { [self] application, error in
                guard isCurrent(request) else { return }
                guard application != nil, error == nil else { finish("Error: Could not open the selected music app", request: request); return }
                guard play else { finish("Music app opened", request: request); return }
                worker.async { [self] in
                    do {
                        for _ in 0..<20 {
                            guard isCurrent(request) else { return }
                            if let app = NSRunningApplication.runningApplications(withBundleIdentifier: bundle).first,
                               let player = mediaSurface(playerRoot(app.processIdentifier), service: route) {
                                if !player.playing { try pressAX(player.toggle) }
                                for _ in 0..<8 {
                                    guard isCurrent(request) else { return }
                                    if mediaSurface(playerRoot(app.processIdentifier), service: route)?.playing == true {
                                        finish("\(route == "spotify" ? "Spotify" : "Apple Music"): playing", request: request); return
                                    }
                                    Thread.sleep(forTimeInterval: 0.25)
                                }
                                throw MusicError.failed("Choose a track in the selected music app first")
                            }
                            Thread.sleep(forTimeInterval: 0.5)
                        }
                        throw MusicError.failed("Choose a track or sign in to the selected music app, then try again")
                    } catch { finish("Error: \(error)", request: request) }
                }
            }
        }
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
                            guard let root = playerWindow(running.processIdentifier) else {
                                Thread.sleep(forTimeInterval: 0.75)
                                continue
                            }
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
            // Send Launch Services a reopen request even if the web-app process
            // survived its red close button. Activating that process alone does
            // not recreate a closed Safari web-app window.
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
