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
    var fill: Color = Theme.Colors.field
    var textColor: Color = Theme.Colors.primaryText
    var symbol: String? = nil
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
            if let symbol { Image(systemName: symbol).font(Theme.Fonts.meta).imageScale(.small) }
            Text(title)
            if let dot { Circle().fill(dot).frame(width: 6, height: 6) }
        }
        .font(Theme.Fonts.smallStrong).foregroundStyle(textColor)
        .padding(.horizontal, 9).padding(.vertical, 3)
        .background(Capsule().fill(fill))
    }
}

struct PillButton: View {
    let title: String
    var fill: Color = Theme.Colors.field
    var textColor: Color = Theme.Colors.primaryText
    var symbol: String? = nil
    var icon: NSImage? = nil
    var showsIcon = false
    var dot: Color? = nil
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            PillLabel(title: title, fill: fill, textColor: textColor, symbol: symbol, icon: icon, showsIcon: showsIcon, dot: dot)
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
        let textColor = tint.map { Color(nsColor: NSColor($0).blended(withFraction: 0.35, of: .white) ?? .white) } ?? Theme.Colors.secondaryText
        Text(text).font(Theme.Fonts.metaStrong)
            .foregroundStyle(textColor)
            .lineLimit(1).truncationMode(.tail)
            .frame(maxWidth: 130 - 14).fixedSize(horizontal: true, vertical: false)
            .padding(.horizontal, 7).padding(.vertical, 2)
            .background(Capsule().fill(tint?.opacity(0.25) ?? Theme.Colors.field))
    }
}

/// One line of the inline menu list.
final class RowHover: ObservableObject { @Published var on = false }

struct LinkRow: View {
    let item: LinkItem
    let icon: NSImage?
    let onOpen: () -> Void
    @StateObject private var hover = RowHover()

    var body: some View {
        HStack(spacing: 6) {
            leadingIcon.frame(width: 14, height: 14)
            Text(item.code).font(Theme.Fonts.metaMono).foregroundStyle(Theme.Colors.tertiaryText).lineLimit(1).fixedSize()
            if let title = item.title {
                Text(title).font(Theme.Fonts.body).foregroundStyle(Theme.Colors.primaryText).lineLimit(1).truncationMode(.tail)
                    .layoutPriority(1)
            }
            Spacer(minLength: 6)
            if let status = item.status { StatusTag(text: status, tone: item.tone) }
        }
        .padding(.horizontal, 4)
        .frame(height: Layout.linkRowHeight)
        .background(RoundedRectangle(cornerRadius: 5).fill(hover.on ? Theme.Colors.cardHover : Color.clear))
        .contentShape(Rectangle())
        .onHover { hover.on = $0 }
        .onTapGesture(perform: onOpen)
        .help(item.help ?? item.text)
    }

    @ViewBuilder private var leadingIcon: some View {
        switch item.kind {
        case .ticket:
            if let icon { Image(nsImage: icon).resizable().interpolation(.high) } else { Color.clear }
        case .pr: Image(systemName: Theme.Symbols.pr).font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.secondaryText)
        case .link: Image(systemName: "link").font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.secondaryText)
        }
    }
}
