import AppKit
import Foundation

struct YouTubeApp: Codable {
    let id: String
    let name: String
}

func isYouTubeManifest(_ info: [String: Any]) -> Bool {
    guard let manifest = info["Manifest"] as? [String: Any],
          let start = manifest["start_url"] as? String,
          let url = URL(string: start), url.scheme == "https", url.host == "music.youtube.com",
          let identifier = info["CFBundleIdentifier"] as? String else { return false }
    return identifier.hasPrefix("com.apple.Safari.WebApp.")
}

func youtubeApps() -> [YouTubeApp] {
    let folders = [FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Applications"), URL(fileURLWithPath: "/Applications")]
    var apps: [String: YouTubeApp] = [:]
    for folder in folders {
        let candidates = (try? FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: nil, options: [.skipsHiddenFiles])) ?? []
        for candidate in candidates where candidate.pathExtension == "app" {
            guard let data = try? Data(contentsOf: candidate.appendingPathComponent("Contents/Info.plist")),
                  let info = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any],
                  isYouTubeManifest(info), let id = info["CFBundleIdentifier"] as? String else { continue }
            let name = info["CFBundleDisplayName"] as? String ?? info["CFBundleName"] as? String ?? candidate.deletingPathExtension().lastPathComponent
            apps[id] = YouTubeApp(id: id, name: name)
        }
    }
    return apps.values.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
}
