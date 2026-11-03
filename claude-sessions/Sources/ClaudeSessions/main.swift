import AppKit
import ClaudeSessionsKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    let store = Store()
    var panel: WidgetPanel!
    var timer: Timer?
    var monitors: [Any] = []

    func applicationDidFinishLaunching(_ notification: Notification) {
        panel = WidgetPanel(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        panel.hidesOnDeactivate = false
        panel.ignoresMouseEvents = true
        panel.acceptsMouseMovedEvents = true
        let host = DragHostingView(rootView: WidgetView(store: store))
        host.onDragEnd = { [weak self] in self?.settle() }
        panel.contentView = host

        let d = UserDefaults.standard
        if d.object(forKey: "pillX") != nil {
            place(pillOrigin: CGPoint(x: d.double(forKey: "pillX"), y: d.double(forKey: "pillY")))
        } else if let s = NSScreen.main?.visibleFrame {
            place(pillOrigin: CGPoint(x: s.maxX - Layout.collapsed.width - 16, y: s.maxY - Layout.collapsed.height - 10))
        }

        NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { [weak self] _ in
            self?.settle()
        }
        let track: (NSEvent) -> Void = { [weak self] _ in self?.trackMouse() }
        if let m = NSEvent.addGlobalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged], handler: track) { monitors.append(m) }
        if let m = NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged], handler: { track($0); return $0 }) { monitors.append(m) }

        store.refresh()
        panel.orderFrontRegardless()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            self?.store.refresh()
            self?.trackMouse()
        }
    }

    func place(pillOrigin raw: CGPoint) {
        let center = CGPoint(x: raw.x + Layout.collapsed.width / 2, y: raw.y + Layout.collapsed.height / 2)
        let screen = NSScreen.screens.first { $0.frame.contains(center) } ?? NSScreen.main
        guard let s = screen?.frame else { return }
        let o = Layout.clamp(pillOrigin: raw, to: s)
        store.anchor = Anchor.near(CGPoint(x: o.x + Layout.collapsed.width / 2, y: o.y + Layout.collapsed.height / 2), in: s)
        panel.setFrame(Layout.canvasFrame(pillOrigin: o, anchor: store.anchor), display: true)
        UserDefaults.standard.set(Double(o.x), forKey: "pillX")
        UserDefaults.standard.set(Double(o.y), forKey: "pillY")
    }

    func settle() {
        let size = Layout.size(store)
        let pill = Layout.pill(in: panel.frame, size: size, anchor: store.anchor)
        let x: CGFloat
        switch store.anchor.h {
        case .leading: x = pill.minX
        case .center: x = pill.midX - Layout.collapsed.width / 2
        case .trailing: x = pill.maxX - Layout.collapsed.width
        }
        let y = store.anchor.up ? pill.minY : pill.maxY - Layout.collapsed.height
        place(pillOrigin: CGPoint(x: x, y: y))
        trackMouse()
    }

    func trackMouse() {
        let zone = Layout.pill(in: panel.frame, size: Layout.size(store), anchor: store.anchor)
        let inside = zone.contains(NSEvent.mouseLocation)
        if inside != store.hovering {
            store.hovering = inside
            if inside { store.peekUntil = nil }
        }
        panel.ignoresMouseEvents = !inside
    }
}


let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let delegate = AppDelegate()
app.delegate = delegate
app.run()
