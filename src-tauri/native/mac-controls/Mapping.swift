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

func updateKnobMappings(target: UInt64?, directory: URL) throws -> [ConnectedKeyboard] {
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let lockPath = directory.appendingPathComponent("native-map.lock").path
    let descriptor = Darwin.open(lockPath, O_CREAT | O_RDWR, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw MusicError.failed("Cannot open Mac mapping lock") }
    defer { Darwin.close(descriptor) }
    guard flock(descriptor, LOCK_EX) == 0 else { throw MusicError.failed("Cannot lock Mac mappings") }
    defer { flock(descriptor, LOCK_UN) }
    let ownershipFile = directory.appendingPathComponent("knob-ownership.json")
    var owners: [String: MuteOwnership] = [:]
    if FileManager.default.fileExists(atPath: ownershipFile.path) {
        owners = try JSONDecoder().decode([String: MuteOwnership].self, from: Data(contentsOf: ownershipFile))
    }
    let listing = try runHidutil(["list", "--ndjson", "--matching", "{\"VendorID\":3141}"])
    let services = try listing.split(separator: "\n").compactMap { line -> [String: Any]? in
        let value = try JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any]
        return value?["type"] as? String == "service" ? value : nil
    }
    var connected: [ConnectedKeyboard] = []
    var changes: [(KeyboardProfile, [KeyMapping], [KeyMapping], MuteOwnership?)] = []
    for profile in keyboardProfiles {
        let matches = services.filter { service in profile.matching.allSatisfy { key, value in String(describing: service[key] ?? "") == String(describing: value) } }
        guard matches.count <= 1 else { throw MusicError.failed("Multiple matching AJAZZ services; no changes made") }
        guard matches.count == 1 else { continue }
        connected.append(ConnectedKeyboard(profile: profile.id, product: profile.matching["Product"] as! String, transport: profile.id == "usb" ? "USB" : "Bluetooth"))
        let match = try encodeJSON(profile.matching)
        let current = try parseMappings(runHidutil(["property", "--matching", match, "--get", "UserKeyMapping"]))
        let (desired, owner) = try plannedMute(current, owner: owners[profile.id], target: target)
        changes.append((profile, current, desired, owner))
    }
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
    // Persist recovery records before writing, and retain old records until
    // their undo has succeeded. No keyboard firmware commands are involved.
    for (profile, _, _, owner) in changes { if let owner { owners[profile.id] = owner } }
    try encoder.encode(owners).write(to: ownershipFile, options: .atomic)
    for (profile, current, desired, _) in changes where current != desired {
        let match = try encodeJSON(profile.matching)
        _ = try runHidutil(["property", "--matching", match, "--set", encodeJSON(["UserKeyMapping": desired])])
        guard try parseMappings(runHidutil(["property", "--matching", match, "--get", "UserKeyMapping"])) == desired else { throw MusicError.failed("Mac knob mapping readback failed") }
    }
    for (profile, _, _, owner) in changes { owners[profile.id] = owner }
    try encoder.encode(owners).write(to: ownershipFile, options: .atomic)
    return connected
}
