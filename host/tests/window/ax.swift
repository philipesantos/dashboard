// The accessibility tree of a host window through the AX API, for host/tests/window.rs on macOS.
// Usage: swift ax.swift dump PID TITLE | find PID TITLE NAME
// `dump` prints one line per element: `role<TAB>name`; `find` prints `x y width height` of the
// first element named NAME. Whether this process may read other apps' trees goes to stderr.
import ApplicationServices
import Foundation

let arguments = CommandLine.arguments
let command = arguments[1]
let pid = pid_t(arguments[2])!
let title = arguments[3]
FileHandle.standardError.write("ax.swift: trusted \(AXIsProcessTrusted())\n".data(using: .utf8)!)

func attribute(_ element: AXUIElement, _ name: String) -> AnyObject? {
    var value: AnyObject?
    AXUIElementCopyAttributeValue(element, name as CFString, &value)
    return value
}

func label(_ element: AXUIElement) -> String {
    for name in [kAXTitleAttribute, kAXDescriptionAttribute, kAXValueAttribute] {
        if let text = attribute(element, name as String) as? String, !text.isEmpty {
            return text
        }
    }
    return ""
}

func walk(_ element: AXUIElement, depth: Int, visit: (AXUIElement) -> Bool) -> Bool {
    if depth > 40 { return false }
    if visit(element) { return true }
    for child in (attribute(element, kAXChildrenAttribute as String) as? [AXUIElement]) ?? [] {
        if walk(child, depth: depth + 1, visit: visit) { return true }
    }
    return false
}

let app = AXUIElementCreateApplication(pid)
let windows = (attribute(app, kAXWindowsAttribute as String) as? [AXUIElement]) ?? []
FileHandle.standardError.write(
    "ax.swift: windows \(windows.map { attribute($0, kAXTitleAttribute as String) as? String ?? "?" })\n"
        .data(using: .utf8)!)
for window in windows where (attribute(window, kAXTitleAttribute as String) as? String) == title {
    _ = walk(window, depth: 0) { element in
        let role = (attribute(element, kAXRoleAttribute as String) as? String) ?? "?"
        let name = label(element)
        if command == "dump" {
            print("\(role)\t\(name)")
            return false
        }
        guard name == arguments[4] else { return false }
        var point = CGPoint.zero
        var size = CGSize.zero
        if let position = attribute(element, kAXPositionAttribute as String) {
            AXValueGetValue(position as! AXValue, .cgPoint, &point)
        }
        if let extent = attribute(element, kAXSizeAttribute as String) {
            AXValueGetValue(extent as! AXValue, .cgSize, &size)
        }
        print("\(point.x) \(point.y) \(size.width) \(size.height)")
        return true
    }
}
