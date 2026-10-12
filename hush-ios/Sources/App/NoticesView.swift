import SwiftUI
import UIKit

/// Bounded pages keep the full bundled notices readable without one huge accessibility value.
struct NoticesView: View {
    @State private var pages: [String]?
    @State private var pageIndex = 0

    var body: some View {
        Group {
            if let pages {
                VStack(spacing: 0) {
                    NoticesTextView(text: pages[pageIndex])
                        .id(pageIndex)
                        .accessibilityIdentifier("notices-text")
                    HStack {
                        Button("Previous") { pageIndex -= 1 }
                            .disabled(pageIndex == 0)
                            .accessibilityIdentifier("notices-previous")
                        Spacer()
                        Text("Page \(pageIndex + 1) of \(pages.count)")
                            .font(.footnote)
                            .accessibilityIdentifier("notices-page")
                        Spacer()
                        Button("Next") { pageIndex += 1 }
                            .disabled(pageIndex == pages.count - 1)
                            .accessibilityIdentifier("notices-next")
                    }
                    .padding()
                }
            } else {
                Text("Loading").foregroundStyle(.secondary)
            }
        }
        .background(Color.hushBackground)
        .navigationTitle("Third-party notices")
        .navigationBarTitleDisplayMode(.inline)
        .task { if pages == nil { pages = await Self.load() } }
    }

    private static func load() async -> [String] {
        await Task.detached(priority: .userInitiated) {
            guard let url = Bundle.main.url(forResource: "ThirdPartyNotices", withExtension: "txt"),
                  let contents = try? String(contentsOf: url, encoding: .utf8)
            else { return ["The notices file is missing from this build."] }
            var pages = [String]()
            var start = contents.startIndex
            while start < contents.endIndex {
                let end = contents.index(start, offsetBy: 8_192, limitedBy: contents.endIndex) ?? contents.endIndex
                pages.append(String(contents[start..<end]))
                start = end
            }
            return pages.isEmpty ? [contents] : pages
        }.value
    }
}

private struct NoticesTextView: UIViewRepresentable {
    let text: String

    func makeUIView(context: Context) -> UITextView {
        let view = UITextView()
        view.isEditable = false
        view.isSelectable = true
        view.backgroundColor = .clear
        view.adjustsFontForContentSizeCategory = true
        view.font = UIFontMetrics(forTextStyle: .footnote).scaledFont(for: .monospacedSystemFont(ofSize: 12, weight: .regular))
        view.textColor = .label
        view.textContainerInset = UIEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
        view.text = text
        return view
    }

    func updateUIView(_ view: UITextView, context: Context) {
        if view.text != text { view.text = text }
    }
}
