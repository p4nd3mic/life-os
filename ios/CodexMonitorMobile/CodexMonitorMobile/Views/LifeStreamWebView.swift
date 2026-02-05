import SwiftUI
import WebKit
import CodexMonitorModels

struct LifeStreamWebView: View {
    @EnvironmentObject private var store: CodexStore

    var body: some View {
        LifeStreamWebViewRepresentable(store: store)
    }
}

private struct LifeStreamWebViewRepresentable: UIViewRepresentable {
    let store: CodexStore

    func makeCoordinator() -> Coordinator {
        Coordinator(store: store)
    }

    func makeUIView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.defaultWebpagePreferences.preferredContentMode = .desktop
        let controller = WKUserContentController()
        controller.add(context.coordinator.bridge, name: "codexBridge")
        configuration.userContentController = controller
        configuration.setURLSchemeHandler(context.coordinator.schemeHandler, forURLScheme: "codex")

        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.isOpaque = false
        webView.backgroundColor = .clear
        webView.scrollView.backgroundColor = .clear
        webView.scrollView.bounces = false
        webView.scrollView.alwaysBounceVertical = false
        webView.scrollView.alwaysBounceHorizontal = false
        webView.scrollView.contentInsetAdjustmentBehavior = .never
        webView.scrollView.keyboardDismissMode = .interactive
        webView.allowsBackForwardNavigationGestures = false
        webView.allowsLinkPreview = false

        context.coordinator.bridge.attach(webView)

        if let url = Bundle.main.url(
            forResource: "index.webview",
            withExtension: "html",
            subdirectory: "WebView"
        ) {
            webView.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        }

        return webView
    }

    func updateUIView(_ uiView: WKWebView, context: Context) {}

    final class Coordinator {
        let bridge: CodexWebViewBridge
        let schemeHandler: CodexWebViewSchemeHandler

        init(store: CodexStore) {
            bridge = CodexWebViewBridge(store: store)
            schemeHandler = CodexWebViewSchemeHandler(store: store)
        }
    }
}

private struct BridgeRequest: Decodable {
    let id: String
    let method: String
    let payload: JSONValue?
}

private struct BridgeResponse: Encodable {
    let id: String
    let ok: Bool
    let result: JSONValue?
    let error: String?
}

final class CodexWebViewBridge: NSObject, WKScriptMessageHandler {
    private weak var webView: WKWebView?
    private let store: CodexStore
    private let encoder = JSONEncoder()

    init(store: CodexStore) {
        self.store = store
        super.init()
        store.lifeStreamEventHandler = { [weak self] event in
            self?.dispatchLifeStreamEvent(event)
        }
    }

    func attach(_ webView: WKWebView) {
        self.webView = webView
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.name == "codexBridge" else { return }
        guard let body = message.body as? [String: Any],
              let data = try? JSONSerialization.data(withJSONObject: body),
              let request = try? JSONDecoder().decode(BridgeRequest.self, from: data) else {
            return
        }

        Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                let result = try await store.callRaw(method: request.method, params: request.payload)
                sendResponse(id: request.id, ok: true, result: result, error: nil)
            } catch {
                sendResponse(id: request.id, ok: false, result: nil, error: error.localizedDescription)
            }
        }
    }

    private func sendResponse(id: String, ok: Bool, result: JSONValue?, error: String?) {
        guard let webView else { return }
        let response = BridgeResponse(id: id, ok: ok, result: result, error: error)
        guard let data = try? encoder.encode(response),
              let json = String(data: data, encoding: .utf8) else {
            return
        }
        DispatchQueue.main.async {
            webView.evaluateJavaScript("window.__codexBridge__?.onNativeMessage(\(json));", completionHandler: nil)
        }
    }

    private func dispatchLifeStreamEvent(_ event: LifeStreamEvent) {
        guard let webView else { return }
        guard let data = try? encoder.encode(event),
              let json = String(data: data, encoding: .utf8) else {
            return
        }
        DispatchQueue.main.async {
            webView.evaluateJavaScript(
                "window.dispatchEvent(new CustomEvent(\"life_stream_event\", { detail: \(json) }));",
                completionHandler: nil
            )
        }
    }
}

final class CodexWebViewSchemeHandler: NSObject, WKURLSchemeHandler {
    private let store: CodexStore

    init(store: CodexStore) {
        self.store = store
    }

    func webView(_ webView: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        guard let url = urlSchemeTask.request.url,
              let components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
            urlSchemeTask.didFailWithError(NSError(domain: "CodexWebView", code: -1))
            return
        }

        let items = components.queryItems ?? []
        let pathValue = items.first(where: { $0.name == "path" })?.value?.removingPercentEncoding
        let workspaceId = items.first(where: { $0.name == "workspaceId" })?.value
            ?? store.activeWorkspaceId
            ?? store.workspaces.first?.id

        guard let pathValue, let workspaceId else {
            urlSchemeTask.didFailWithError(NSError(domain: "CodexWebView", code: -2))
            return
        }

        Task { @MainActor in
            do {
                let asset = try await store.lifeStreamReadAsset(workspaceId: workspaceId, path: pathValue)
                guard let data = Data(base64Encoded: asset.base64) else {
                    urlSchemeTask.didFailWithError(NSError(domain: "CodexWebView", code: -3))
                    return
                }
                let response = URLResponse(
                    url: url,
                    mimeType: asset.mime,
                    expectedContentLength: data.count,
                    textEncodingName: nil
                )
                urlSchemeTask.didReceive(response)
                urlSchemeTask.didReceive(data)
                urlSchemeTask.didFinish()
            } catch {
                urlSchemeTask.didFailWithError(error)
            }
        }
    }

    func webView(_ webView: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {}
}
