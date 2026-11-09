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
    /// While non-nil the pill shows the state of its send instead of its own title and colours.
    var send: SendState? = nil
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            switch send {
            case nil:
                PillLabel(title: title, fill: fill, textColor: textColor, symbol: symbol, icon: icon, showsIcon: showsIcon)
            case .sending?:
                PillLabel(title: SendState.sending.title).opacity(Theme.Opacity.disabled)
            case .sent?:
                PillLabel(title: SendState.sent.title, fill: Theme.Colors.primaryAction, textColor: Theme.Colors.onPrimary)
            case .failed?:
                PillLabel(title: SendState.failed.title, fill: .red, textColor: .white)
            }
        }
        .buttonStyle(.plain)
        .disabled(send == .sending)
    }
}

extension SendState {
    var title: String {
        switch self {
        case .sending: "Đang gửi…"
        case .sent: "Đã gửi ✓"
        case .failed: "Lỗi — bấm thử lại"
        }
    }

    /// The text colour of a `SentLine`.
    var color: Color {
        switch self {
        case .sending: Theme.Colors.secondaryText
        case .sent: Theme.Colors.primaryAction
        case .failed: .red
        }
    }
}

/// One-line placeholder where a send button vanished (the approval block, `Nhắc duyệt`) while its result still shows.
struct SentLine: View {
    let state: SendState

    var body: some View {
        Text(state.title).font(Theme.Fonts.small).foregroundStyle(state.color)
    }
}

/// A 24x24 rounded icon button; `tint` replaces the field fill (the armed `Dừng`).
struct IconButton: View {
    let symbol: String
    let help: String
    var tint: Color? = nil
    /// While non-nil the icon shows the state of its send: spinner, checkmark or red tint.
    var send: SendState? = nil
    let action: () -> Void

    init(symbol: String, help: String, tint: Color? = nil, send: SendState? = nil, action: @escaping () -> Void) {
        self.symbol = symbol
        self.help = help
        self.tint = tint
        self.send = send
        self.action = action
    }

    var body: some View {
        Button(action: action) {
            Group {
                switch send {
                case .sending?: ProgressView().controlSize(.mini)
                case .sent?: Image(systemName: Theme.Symbols.approved).font(Theme.Fonts.small)
                case nil, .failed?: Image(systemName: symbol).font(Theme.Fonts.small)
                }
            }
            .foregroundStyle(Theme.Colors.primaryText)
            .frame(width: 24, height: 24)
            .background(RoundedRectangle(cornerRadius: 6).fill(send == .failed ? Color.red.opacity(0.6) : tint ?? Theme.Colors.field))
        }
        .buttonStyle(.plain)
        .disabled(send == .sending)
        .help(help)
        .accessibilityLabel(help)
    }
}

/// A small capsule label; with `tint` it is tinted and its text brightened. `maxWidth` truncates a long text (ticket and PR status).
struct Tag: View {
    let text: String
    var symbol: String? = nil
    var tint: Color? = nil
    var tintOpacity = Theme.Opacity.tagTint
    var maxWidth: CGFloat? = nil

    init(_ text: String, symbol: String? = nil, tint: Color? = nil, tintOpacity: Double = Theme.Opacity.tagTint, maxWidth: CGFloat? = nil) {
        self.text = text
        self.symbol = symbol
        self.tint = tint
        self.tintOpacity = tintOpacity
        self.maxWidth = maxWidth
    }

    var body: some View {
        let textColor = tint.map { Color(nsColor: NSColor($0).blended(withFraction: 0.35, of: .white) ?? .white) } ?? Theme.Colors.secondaryText
        HStack(spacing: 3) {
            if let symbol { Image(systemName: symbol).font(Theme.Fonts.micro) }
            label
        }
        .font(Theme.Fonts.metaStrong).foregroundStyle(textColor)
        .padding(.horizontal, 7).padding(.vertical, 2)
        .background(Capsule().fill(tint?.opacity(tintOpacity) ?? Theme.Colors.field))
    }

    @ViewBuilder private var label: some View {
        if let maxWidth {
            Text(text).lineLimit(1).truncationMode(.tail)
                .frame(maxWidth: maxWidth).fixedSize(horizontal: true, vertical: false)
        } else {
            Text(text).lineLimit(1)
        }
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
            if let status = item.status {
                Tag(status, tint: toneColor(item.tone), tintOpacity: Theme.Opacity.statusTagTint, maxWidth: 130 - 14)
            }
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
