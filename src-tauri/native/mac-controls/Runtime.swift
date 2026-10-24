import Foundation
import Darwin

enum NativeLockError: Error { case busy, unavailable }

// Never block AppKit on a lock: Process.waitUntilExit can run a nested event
// loop, allowing a timer or workspace observer to enter the same transaction.
final class NativeFileLock {
    let descriptor: Int32
    init(_ path: URL) throws {
        let fd = Darwin.open(path.path, O_CREAT | O_RDWR | O_CLOEXEC, S_IRUSR | S_IWUSR)
        guard fd >= 0 else { throw NativeLockError.unavailable }
        guard flock(fd, LOCK_EX | LOCK_NB) == 0 else {
            let code = errno
            Darwin.close(fd)
            if code == EWOULDBLOCK || code == EAGAIN { throw NativeLockError.busy }
            throw NativeLockError.unavailable
        }
        descriptor = fd
    }
    deinit { flock(descriptor, LOCK_UN); Darwin.close(descriptor) }
}

let showSettingsNotification = Notification.Name("io.github.wsclx.ak820pro-modder.show-settings")
let commandNotification = Notification.Name("io.github.wsclx.ak820pro-modder.command")

/// `--command group:value` (e.g. `lighting:off`, `lighting:restore`,
/// `colour_profile:Work`, `knob_mode:smart`) for Shortcuts and scripts.
func launchCommand(_ arguments: [String] = CommandLine.arguments) -> String? {
    guard let index = arguments.firstIndex(of: "--command"), index + 1 < arguments.count else { return nil }
    let command = arguments[index + 1]
    return command.count <= 64 && command.contains(":") ? command : nil
}
private var applicationLease: NativeFileLock?

// Called before creating a webview or registering hotkeys. Different copies of
// this app must share one controller, including login/background launches.
@_cdecl("ak820_mac_claim_instance")
public func ak820MacClaimInstance(_ showWindow: Int32) -> Int32 {
    let directory = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/io.github.wsclx.ak820pro-modder", isDirectory: true)
    do {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        applicationLease = try NativeFileLock(directory.appendingPathComponent("application-instance.lock"))
        return 1
    } catch NativeLockError.busy {
        if let command = launchCommand() {
            DistributedNotificationCenter.default().postNotificationName(commandNotification, object: command, userInfo: nil, deliverImmediately: true)
        } else if showWindow != 0 {
            DistributedNotificationCenter.default().postNotificationName(showSettingsNotification, object: nil, userInfo: nil, deliverImmediately: true)
        }
        return 0
    } catch {
        fputs("Cannot acquire AK820 application lock: \(error)\n", stderr)
        return -1
    }
}
