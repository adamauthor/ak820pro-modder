import Foundation
import Darwin

let muteUsage: UInt64 = (12 << 32) | 0xe2
let playUsage: UInt64 = (12 << 32) | 0xcd
let triggerUsage: UInt64 = (7 << 32) | 0x6f
let mappingSource = "HIDKeyboardModifierMappingSrc"
let mappingDestination = "HIDKeyboardModifierMappingDst"
typealias KeyMapping = [String: UInt64]

struct KeyboardProfile {
    let id: String
    let matching: [String: Any]
}
let keyboardProfiles = [
    KeyboardProfile(id: "usb", matching: ["VendorID": 3141, "ProductID": 32778, "Product": "AK820", "Transport": "USB", "PrimaryUsagePage": 12, "PrimaryUsage": 1]),
] + (1...3).map { slot in
    KeyboardProfile(id: "bluetooth-\(slot)", matching: ["VendorID": 3141, "ProductID": 65278, "Product": "AK820 5.1-\(slot)", "Transport": "Bluetooth Low Energy", "PrimaryUsagePage": 1, "PrimaryUsage": 6])
}
struct ConnectedKeyboard: Codable {
    let profile: String
    let product: String
    let transport: String
}
struct MuteOwnership: Codable, Equatable {
    let original: UInt64?
    let applied: UInt64
}

func encodeJSON(_ object: Any) throws -> String {
    String(decoding: try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys]), as: UTF8.self)
}
func runHidutil(_ arguments: [String]) throws -> String {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/hidutil")
    process.arguments = arguments
    let output = Pipe(), errors = Pipe()
    process.standardOutput = output
    process.standardError = errors
    try process.run()
    let data = output.fileHandleForReading.readDataToEndOfFile()
    let errorData = errors.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    guard process.terminationStatus == 0 else { throw MusicError.failed("hidutil: \(String(decoding: errorData, as: UTF8.self))") }
    return String(decoding: data, as: UTF8.self)
}
func parseMappings(_ output: String) throws -> [KeyMapping] {
    let parts = output.components(separatedBy: "UserKeyMapping")
    guard parts.count == 2 else { throw MusicError.failed("Missing or ambiguous AJAZZ media service") }
    let value = parts[1].trimmingCharacters(in: .whitespacesAndNewlines)
    if value == "(null)" { return [] }
    guard let rows = try PropertyListSerialization.propertyList(from: Data(value.utf8), options: [], format: nil) as? [[String: Any]] else { throw MusicError.failed("Invalid media mapping") }
    return try rows.map { row in
        guard Set(row.keys) == Set([mappingSource, mappingDestination]),
              let source = row[mappingSource], let destination = row[mappingDestination],
              let src = UInt64(String(describing: source)), let dst = UInt64(String(describing: destination)) else { throw MusicError.failed("Unknown mapping fields; no changes made") }
        return [mappingSource: src, mappingDestination: dst]
    }
}

// Only the Consumer Mute rule belongs to the knob. Keyboard-page entries on
// the combined Bluetooth service must survive every change and undo.
func plannedMute(_ current: [KeyMapping], owner: MuteOwnership?, target: UInt64?) throws -> ([KeyMapping], MuteOwnership?) {
    let rules = current.filter { $0[mappingSource] == muteUsage }
    guard rules.count <= 1 else { throw MusicError.failed("Duplicate Mute mappings; no changes made") }
    let existing = rules.first?[mappingDestination]
    if let owner, let existing, existing != owner.applied && existing != owner.original {
        throw MusicError.failed("Mute was changed by another app; leaving it unchanged")
    }
    guard target != nil || owner != nil else { return (current, nil) }
    let desired = target ?? owner?.original
    let others = current.filter { $0[mappingSource] != muteUsage }
    let result = desired.map { others + [[mappingSource: muteUsage, mappingDestination: $0]] } ?? others
    let original: UInt64?
    if let owner { original = owner.original } else { original = existing }
    let nextOwner = target.map { MuteOwnership(original: original, applied: $0) }
    return (result, nextOwner)
}

// Version 2 tracks each owned source separately. A legacy Mute record is migrated
// in memory; the original destination (including absence) is never guessed.
struct MappingOwnership: Codable {
    var version = 2
    var profiles: [String: [String: MuteOwnership]] = [:]
}
func decodeOwnership(_ data: Data) throws -> MappingOwnership {
    // A newer Modder may have written a format this build cannot interpret.
    // Refuse instead of guessing so its recovery records stay intact.
    if let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
       let version = object["version"] as? Int, version > 2 {
        throw MusicError.failed("Mapping records were saved by a newer Modder; no changes made")
    }
    if let current = try? JSONDecoder().decode(MappingOwnership.self, from: data), current.version == 2 {
        for rules in current.profiles.values {
            for key in rules.keys {
                guard let source = UInt64(key), [muteUsage, volumeUpUsage, volumeDownUsage].contains(source) || source >> 32 == 7 && (4...231).contains(source & 0xffffffff) else { throw MusicError.failed("Invalid mapping recovery source; no changes made") }
            }
        }
        return current
    }
    let old = try JSONDecoder().decode([String: MuteOwnership].self, from: data)
    return MappingOwnership(profiles: old.mapValues { [String(muteUsage): $0] })
}
func plannedRules(_ current: [KeyMapping], owners: [String: MuteOwnership], targets: [UInt64: UInt64]) throws -> ([KeyMapping], [String: MuteOwnership]) {
    var result = current
    var next: [String: MuteOwnership] = [:]
    for source in Set(owners.keys.compactMap(UInt64.init)).union(targets.keys).sorted() {
        let key = String(source), owner = owners[String(source)]
        let rows = current.filter { $0[mappingSource] == source }
        guard rows.count <= 1 else { throw MusicError.failed("Duplicate keyboard mapping; no changes made") }
        let existing = rows.first?[mappingDestination]
        if let owner, let existing, existing != owner.applied && existing != owner.original {
            if targets[source] == nil { continue } // An external editor owns it now. Preserve it during undo.
            throw MusicError.failed("A key mapping was changed by another app; no changes made")
        }
        let desired: UInt64?
        if let target = targets[source] {
            desired = target
            next[key] = MuteOwnership(original: owner != nil ? owner!.original : existing, applied: target)
        } else { desired = owner?.original }
        result.removeAll { $0[mappingSource] == source }
        if let desired { result.append([mappingSource: source, mappingDestination: desired]) }
    }
    return (result, next)
}
let volumeUpUsage: UInt64 = (12 << 32) | 0xe9
let volumeDownUsage: UInt64 = (12 << 32) | 0xea
let turnUpUsage: UInt64 = (7 << 32) | 109
let turnDownUsage: UInt64 = (7 << 32) | 110
let usbKeys = KeyboardProfile(id: "usb-keys", matching: ["VendorID": 3141, "ProductID": 32778, "Product": "AK820", "Transport": "USB", "PrimaryUsagePage": 1, "PrimaryUsage": 6])

func updateControlMappings(target: UInt64?, rotation: Bool, remaps: [ProfileRemap], base: [ProfileRemap] = [], directory: URL) throws -> [ConnectedKeyboard] {
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let lease: NativeFileLock
    do { lease = try NativeFileLock(directory.appendingPathComponent("native-map.lock")) }
    catch NativeLockError.busy { throw MusicError.failed("Mac mappings are busy; retry in a moment") }
    catch { throw MusicError.failed("Cannot lock Mac mappings") }
    return try withExtendedLifetime(lease) {
        try updateLockedMappings(target: target, rotation: rotation, remaps: remaps, base: base, directory: directory)
    }
}
private func updateLockedMappings(target: UInt64?, rotation: Bool, remaps: [ProfileRemap], base: [ProfileRemap], directory: URL) throws -> [ConnectedKeyboard] {
    let ownershipFile = directory.appendingPathComponent("knob-ownership.json")
    var document = FileManager.default.fileExists(atPath: ownershipFile.path) ? try decodeOwnership(Data(contentsOf: ownershipFile)) : MappingOwnership()
    let listing = try runHidutil(["list", "--ndjson", "--matching", "{\"VendorID\":3141}"])
    let services = try listing.split(separator: "\n").compactMap { line -> [String: Any]? in
        let value = try JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any]
        return value?["type"] as? String == "service" ? value : nil
    }
    var connected: [ConnectedKeyboard] = []
    var changes: [(KeyboardProfile, [KeyMapping], [KeyMapping], [String: MuteOwnership])] = []
    for profile in keyboardProfiles + [usbKeys] {
        let matches = services.filter { service in profile.matching.allSatisfy { key, value in String(describing: service[key] ?? "") == String(describing: value) } }
        guard matches.count <= 1 else { throw MusicError.failed("Multiple matching AJAZZ services; no changes made") }
        guard matches.count == 1 else { continue }
        if profile.id != "usb-keys" { connected.append(ConnectedKeyboard(profile: profile.id, product: profile.matching["Product"] as! String, transport: profile.id == "usb" ? "USB" : "Bluetooth")) }
        let match = try encodeJSON(profile.matching)
        let current = try parseMappings(runHidutil(["property", "--matching", match, "--get", "UserKeyMapping"]))
        var targets: [UInt64: UInt64] = [:]
        if profile.id != "usb-keys" {
            targets[muteUsage] = target
            if rotation { targets[volumeUpUsage] = turnUpUsage; targets[volumeDownUsage] = turnDownUsage }
        }
        if profile.id != "usb" {
            // Base mappings are the lowest layer; an app profile overrides them.
            for r in layeredRemaps(base: base, profile: remaps) { targets[(7 << 32) | UInt64(r.source)] = (7 << 32) | UInt64(r.destination) }
        }
        // Reserved marker destinations must never already belong to normal keys.
        let previous = document.profiles[profile.id] ?? [:]
        for row in current where [triggerUsage, turnUpUsage, turnDownUsage].contains(row[mappingDestination] ?? 0) {
            let reserved = (target == triggerUsage ? [triggerUsage] : []) + (rotation ? [turnUpUsage, turnDownUsage] : [])
            if let source = row[mappingSource], reserved.contains(row[mappingDestination]!), previous[String(source)] == nil {
                throw MusicError.failed("F18–F20 are already assigned to another key. Remove that mapping first")
            }
        }
        let (desired, owner) = try plannedRules(current, owners: previous, targets: targets)
        changes.append((profile, current, desired, owner))
    }
    let encoder = JSONEncoder(); encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
    let old = document
    for (profile, _, _, owner) in changes {
        document.profiles[profile.id] = (document.profiles[profile.id] ?? [:]).merging(owner) { _, new in new }
    }
    try writeIfChanged(encoder.encode(document), to: ownershipFile)
    var applied: [(KeyboardProfile, [KeyMapping])] = []
    do {
        for (profile, current, desired, _) in changes where current != desired {
            applied.append((profile, current)) // Also undo a successful write followed by failed readback.
            let match = try encodeJSON(profile.matching)
            _ = try runHidutil(["property", "--matching", match, "--set", encodeJSON(["UserKeyMapping": desired])])
            guard try parseMappings(runHidutil(["property", "--matching", match, "--get", "UserKeyMapping"])) == desired else { throw MusicError.failed("Mac control mapping readback failed") }
        }
    } catch {
        var restored = true
        for (profile, current) in applied.reversed() {
            do {
                let match = try encodeJSON(profile.matching)
                _ = try runHidutil(["property", "--matching", match, "--set", encodeJSON(["UserKeyMapping": current])])
                if try parseMappings(runHidutil(["property", "--matching", match, "--get", "UserKeyMapping"])) != current { restored = false }
            } catch { restored = false }
        }
        if restored { try? encoder.encode(old).write(to: ownershipFile, options: .atomic) }
        throw error
    }
    for (profile, _, _, owner) in changes { document.profiles[profile.id] = owner }
    try writeIfChanged(encoder.encode(document), to: ownershipFile)
    return connected
}
// The periodic refresh usually changes nothing; avoid rewriting the file every tick.
func writeIfChanged(_ data: Data, to url: URL) throws {
    if (try? Data(contentsOf: url)) == data { return }
    try data.write(to: url, options: .atomic)
}
func layeredRemaps(base: [ProfileRemap], profile: [ProfileRemap]) -> [ProfileRemap] {
    let overridden = Set(profile.map(\.source))
    return base.filter { !overridden.contains($0.source) } + profile
}

// Crash recovery: restores every recorded original without touching AppKit
// state, so it is safe from a panic hook on any thread.
@_cdecl("ak820_mac_emergency_restore")
public func ak820MacEmergencyRestore() -> Int32 {
    let directory = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/io.github.wsclx.ak820pro-modder", isDirectory: true)
    do { _ = try updateControlMappings(target: nil, rotation: false, remaps: [], directory: directory); return 0 }
    catch { return 1 }
}
func updateKnobMappings(target: UInt64?, directory: URL) throws -> [ConnectedKeyboard] {
    try updateControlMappings(target: target, rotation: false, remaps: [], directory: directory)
}
