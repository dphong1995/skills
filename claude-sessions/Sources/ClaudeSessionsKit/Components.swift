import AppKit
import SwiftUI

/// Tone to a tint: doing blue, done/open green, merged purple; `nil` for grey (todo, closed, no tone).
func toneColor(_ t: ItemTone?) -> Color? {
    switch t {
    case .doing?: .blue
    case .done?, .open?: .green
    case .merged?: .purple
    case .todo?, .closed?, nil: nil
    }
}

struct PillLabel: View {
    let title: String
    var fill: Color = .white.opacity(0.12)
    var textColor: Color = .white
    var icon: NSImage? = nil
    /// Reserves a 14x14 slot for the icon even while `icon` is nil, so labels line up.
    var showsIcon = false
    var dot: Color? = nil

    var body: some View {
        HStack(spacing: 5) {
            if showsIcon {
                Group {
                    if let icon { Image(nsImage: icon).resizable().interpolation(.high) } else { Color.clear }
                }
                .frame(width: 14, height: 14)
            }
            Text(title)
            if let dot { Circle().fill(dot).frame(width: 6, height: 6) }
        }
        .font(.system(size: 11, weight: .semibold)).foregroundStyle(textColor)
        .padding(.horizontal, 9).padding(.vertical, 3)
        .background(Capsule().fill(fill))
    }
}

struct PillButton: View {
    let title: String
    var fill: Color = .white.opacity(0.12)
    var textColor: Color = .white
    var icon: NSImage? = nil
    var showsIcon = false
    var dot: Color? = nil
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            PillLabel(title: title, fill: fill, textColor: textColor, icon: icon, showsIcon: showsIcon, dot: dot)
        }
        .buttonStyle(.plain)
    }
}

/// Status of a ticket or PR: a capsule tinted by its tone.
struct StatusTag: View {
    let text: String
    let tone: ItemTone?

    var body: some View {
        let tint = toneColor(tone)
        let textColor = tint.map { Color(nsColor: NSColor($0).blended(withFraction: 0.35, of: .white) ?? .white) } ?? .white.opacity(0.55)
        Text(text).font(.system(size: 10.5, weight: .semibold))
            .foregroundStyle(textColor)
            .lineLimit(1).truncationMode(.tail)
            .frame(maxWidth: 130 - 14).fixedSize(horizontal: true, vertical: false)
            .padding(.horizontal, 7).padding(.vertical, 2)
            .background(Capsule().fill(tint?.opacity(0.25) ?? .white.opacity(0.12)))
    }
}

/// One line of the inline `☰ N` list.
final class RowHover: ObservableObject { @Published var on = false }

struct LinkRow: View {
    let item: LinkItem
    let icon: NSImage?
    let onOpen: () -> Void
    @StateObject private var hover = RowHover()

    var body: some View {
        HStack(spacing: 6) {
            leadingIcon.frame(width: 14, height: 14)
            Text(item.code).font(.system(size: 10.5).monospaced()).foregroundStyle(.white.opacity(0.5)).lineLimit(1).fixedSize()
            if let title = item.title {
                Text(title).font(.system(size: 11.5)).foregroundStyle(.white.opacity(0.85)).lineLimit(1).truncationMode(.tail)
                    .layoutPriority(1)
            }
            Spacer(minLength: 6)
            if let status = item.status { StatusTag(text: status, tone: item.tone) }
        }
        .padding(.horizontal, 4)
        .frame(height: Layout.linkRowHeight)
        .background(RoundedRectangle(cornerRadius: 5).fill(.white.opacity(hover.on ? 0.08 : 0)))
        .contentShape(Rectangle())
        .onHover { hover.on = $0 }
        .onTapGesture(perform: onOpen)
        .help(item.help ?? item.text)
    }

    @ViewBuilder private var leadingIcon: some View {
        switch item.kind {
        case .ticket:
            if let icon { Image(nsImage: icon).resizable().interpolation(.high) } else { Color.clear }
        case .pr: Image(systemName: "arrow.triangle.pull").font(.system(size: 10)).foregroundStyle(.white.opacity(0.6))
        case .link: Image(systemName: "link").font(.system(size: 10)).foregroundStyle(.white.opacity(0.6))
        }
    }
}
