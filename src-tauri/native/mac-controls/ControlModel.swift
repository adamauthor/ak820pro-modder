import AppKit
import Foundation

struct GestureBindings: Codable, Equatable {
    var single = "toggle"
    var double = "next"
    var triple = "previous"
    var rotation = "volume"
    var shift_rotation = "seek"
}
struct ProfileRemap: Codable, Equatable { let source: UInt16; let destination: UInt16 }
struct AppProfile: Codable, Equatable {
    let id: String
    var name: String
    let bundle_id: String
    var enabled: Bool
    var bindings: GestureBindings
    var remaps: [ProfileRemap]
}
struct ControlConfig: Codable, Equatable {
    var knob_mode: String
    var launch_at_login: Bool
    var youtube_app_id: String?
    var music_service: String
    var follow_playing: Bool
    var capture_rotation: Bool
    var click_interval_ms: Int
    var bindings: GestureBindings
    var profiles: [AppProfile]
    var advanced: Bool { ["youtube_music", "smart"].contains(knob_mode) }
    func validate() throws {
        guard ["original", "play_pause", "youtube_music", "smart"].contains(knob_mode),
              ["youtube_music", "spotify", "apple_music"].contains(music_service),
              (220...500).contains(click_interval_ms), profiles.count <= 32 else { throw MusicError.failed("Invalid Mac controls configuration") }
        let clicks = ["toggle", "next", "previous", "open", "none"]
        let turns = ["volume", "seek", "zoom", "tabs", "scroll", "none"]
        for binding in [bindings] + profiles.map(\.bindings) {
            guard [binding.single, binding.double, binding.triple].allSatisfy(clicks.contains),
                  [binding.rotation, binding.shift_rotation].allSatisfy(turns.contains) else { throw MusicError.failed("Unknown gesture action") }
        }
        var ids = Set<String>(), bundles = Set<String>()
        for p in profiles {
            guard !p.id.isEmpty, p.id.count <= 100, ids.insert(p.id).inserted, bundles.insert(p.bundle_id).inserted,
                  !p.name.trimmingCharacters(in: .whitespaces).isEmpty, p.name.count <= 100,
                  p.bundle_id.count <= 200, p.bundle_id.contains("."), p.remaps.count <= 128 else { throw MusicError.failed("Profiles need unique apps, IDs and valid names") }
            var sources = Set<UInt16>()
            for r in p.remaps {
                guard (4...231).contains(r.source), (4...231).contains(r.destination),
                      ![109, 110, 111].contains(r.source), ![109, 110, 111].contains(r.destination),
                      sources.insert(r.source).inserted else { throw MusicError.failed("Invalid or reserved profile key mapping") }
            }
        }
    }
    func profile(for bundle: String?) -> AppProfile? {
        guard bundle != Bundle.main.bundleIdentifier else { return nil }
        return profiles.first { $0.enabled && $0.bundle_id == bundle }
    }
}

// Monotonic timestamps make the boundary deterministic in fixtures and immune to clock changes.
struct ClickSequence {
    var count = 0
    var last: TimeInterval = 0
    mutating func press(at time: TimeInterval, interval: TimeInterval) -> Int? {
        let expired = count > 0 && time - last >= interval ? count : nil
        if expired != nil { count = 0 }
        count += 1; last = time
        return expired
    }
    mutating func take() -> Int { let result = count; count = 0; return result }
}
struct GestureEvent: Codable {
    let id: Int
    let gesture: String
    let action: String
    let profile: String
    let preview: Bool
}
struct InstalledApp: Codable { let id: String; let name: String }
func installedApplications() -> [InstalledApp] {
    var apps: [String: InstalledApp] = [:]
    let folders = ["/Applications", "/System/Applications", FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Applications").path]
    for folder in folders {
        for url in (try? FileManager.default.contentsOfDirectory(at: URL(fileURLWithPath: folder), includingPropertiesForKeys: nil, options: [.skipsHiddenFiles])) ?? [] where url.pathExtension == "app" {
            guard let bundle = Bundle(url: url), let id = bundle.bundleIdentifier else { continue }
            apps[id] = InstalledApp(id: id, name: bundle.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String ?? bundle.object(forInfoDictionaryKey: "CFBundleName") as? String ?? url.deletingPathExtension().lastPathComponent)
        }
    }
    return apps.values.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
}
