import SwiftUI
import CodexMonitorModels

struct RootView: View {
    @EnvironmentObject private var store: CodexStore
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @State private var showSettings = false

    var body: some View {
        Group {
            if horizontalSizeClass == .compact {
                PhoneRootView(showSettings: $showSettings)
            } else {
                LifeStreamShellView(showSettings: $showSettings)
            }
        }
        .sheet(isPresented: $showSettings) {
            SettingsView()
        }
        .onAppear {
            if store.connectionState == .disconnected {
                store.connect()
            }
        }
    }
}

private struct PhoneRootView: View {
    @EnvironmentObject private var store: CodexStore
    @Binding var showSettings: Bool

    var body: some View {
        TabView {
            NavigationStack {
                ProjectsView()
                    .toolbar {
                        ToolbarItem(placement: .topBarTrailing) {
                            Button(action: { showSettings = true }) {
                                Image(systemName: "gearshape")
                            }
                        }
                    }
            }
            .tabItem {
                Label("Projects", systemImage: "folder")
            }

            NavigationStack {
                DomainDashboardView()
                    .toolbar {
                        ToolbarItem(placement: .topBarTrailing) {
                            Button(action: { showSettings = true }) {
                                Image(systemName: "gearshape")
                            }
                        }
                    }
            }
            .tabItem {
                Label("Domain", systemImage: "rectangle.grid.2x2")
            }

            NavigationStack {
                ConversationTabView()
                    .toolbar {
                        ToolbarItem(placement: .topBarTrailing) {
                            Button(action: { showSettings = true }) {
                                Image(systemName: "gearshape")
                            }
                        }
                    }
            }
            .tabItem {
                Label("Codex", systemImage: "bubble.left.and.text.bubble.right")
            }

            NavigationStack {
                MemoryView()
                    .toolbar {
                        ToolbarItem(placement: .topBarTrailing) {
                            Button(action: { showSettings = true }) {
                                Image(systemName: "gearshape")
                            }
                        }
                    }
            }
            .tabItem {
                Label("Memory", systemImage: "brain.head.profile")
            }

            NavigationStack {
                GitView()
                    .toolbar {
                        ToolbarItem(placement: .topBarTrailing) {
                            Button(action: { showSettings = true }) {
                                Image(systemName: "gearshape")
                            }
                        }
                    }
            }
            .tabItem {
                Label("Git", systemImage: "arrow.triangle.branch")
            }

            NavigationStack {
                DebugLogView()
                    .toolbar {
                        ToolbarItem(placement: .topBarTrailing) {
                            Button(action: { showSettings = true }) {
                                Image(systemName: "gearshape")
                            }
                        }
                    }
            }
            .tabItem {
                Label("Log", systemImage: "waveform.path.ecg")
            }

            NavigationStack {
                BrowserView()
                    .toolbar {
                        ToolbarItem(placement: .topBarTrailing) {
                            Button(action: { showSettings = true }) {
                                Image(systemName: "gearshape")
                            }
                        }
                    }
            }
            .tabItem {
                Label("Browser", systemImage: "safari")
            }

            NavigationStack {
                SkillsView()
                    .toolbar {
                        ToolbarItem(placement: .topBarTrailing) {
                            Button(action: { showSettings = true }) {
                                Image(systemName: "gearshape")
                            }
                        }
                    }
            }
            .tabItem {
                Label("Skills", systemImage: "sparkles")
            }
        }
        .modifier(GlassTabBarStyle())
    }
}

private struct LifeStreamShellView: View {
    @Binding var showSettings: Bool

    var body: some View {
        ZStack(alignment: .topTrailing) {
            LifeStreamWebView()
                .ignoresSafeArea()

            Button(action: { showSettings = true }) {
                Image(systemName: "gearshape")
                    .padding(10)
            }
            .background(.ultraThinMaterial, in: Circle())
            .padding(16)
            .accessibilityLabel("Settings")
        }
    }
}

private struct TabletRootView: View {
    @EnvironmentObject private var store: CodexStore
    @Binding var showSettings: Bool
    @State private var selectedWorkspace: WorkspaceInfo?
    @State private var selectedThreadId: String?
    @State private var detailSelection: TabletDetail = .conversation

    enum TabletDetail: String, CaseIterable {
        case conversation = "Conversation"
        case domain = "Domain"
        case memory = "Memory"
        case git = "Git"
        case files = "Files"
        case prompts = "Prompts"
        case terminal = "Terminal"
        case browser = "Browser"
        case skills = "Skills"
    }

    var body: some View {
        NavigationSplitView {
            WorkspaceListView(selectedWorkspace: $selectedWorkspace)
                .navigationTitle("Workspaces")
                .navigationSplitViewColumnWidth(min: 180, ideal: 210, max: 240)
        } content: {
            ThreadsListView(
                selectedWorkspace: $selectedWorkspace,
                selectedThreadId: $selectedThreadId
            )
            .navigationTitle("Threads")
            .navigationSplitViewColumnWidth(min: 200, ideal: 230, max: 280)
        } detail: {
            DetailColumnView(
                detailSelection: $detailSelection,
                selectedThreadId: selectedThreadId,
                showSettings: $showSettings
            )
        }
        .navigationSplitViewStyle(.balanced)
    }
}

private struct DetailColumnView: View {
    @Binding var detailSelection: TabletRootView.TabletDetail
    let selectedThreadId: String?
    @Binding var showSettings: Bool

    var body: some View {
        VStack(spacing: 0) {
            // Glass segmented picker header
            GlassSegmentedPicker(selection: $detailSelection)
                .padding(.horizontal)
                .padding(.vertical, 12)

            Group {
                switch detailSelection {
                case .conversation:
                    ConversationTabView(selectedThreadId: selectedThreadId)
                case .domain:
                    DomainDashboardView()
                case .memory:
                    MemoryView()
                case .git:
                    GitView()
                case .files:
                    FilesView()
                case .prompts:
                    PromptsView()
                case .terminal:
                    TerminalView()
                case .browser:
                    BrowserView()
                case .skills:
                    SkillsView()
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background {
            GradientBackground()
        }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button(action: { showSettings = true }) {
                    Image(systemName: "gearshape")
                }
            }
        }
    }
}

// MARK: - Glass Segmented Picker
private struct GlassSegmentedPicker: View {
    @Binding var selection: TabletRootView.TabletDetail

    var body: some View {
        if #available(iOS 26.0, *) {
            glassPickerContent
                .glassEffect(.regular, in: .capsule)
        } else {
            glassPickerContent
                .background(.ultraThinMaterial, in: Capsule())
        }
    }

    private var glassPickerContent: some View {
        HStack(spacing: 4) {
            ForEach(TabletRootView.TabletDetail.allCases, id: \.self) { item in
                GlassPickerButton(
                    title: item.rawValue,
                    icon: iconFor(item),
                    isSelected: selection == item
                ) {
                    withAnimation(.easeInOut(duration: 0.2)) {
                        selection = item
                    }
                }
            }
        }
        .padding(4)
    }

    private func iconFor(_ item: TabletRootView.TabletDetail) -> String {
        switch item {
        case .conversation: return "bubble.left.and.text.bubble.right"
        case .domain: return "rectangle.grid.2x2"
        case .memory: return "brain.head.profile"
        case .git: return "arrow.triangle.branch"
        case .files: return "folder"
        case .prompts: return "text.alignleft"
        case .terminal: return "terminal"
        case .browser: return "safari"
        case .skills: return "sparkles"
        }
    }
}

private struct GlassPickerButton: View {
    let title: String
    let icon: String
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Image(systemName: icon)
                    .font(.system(size: 12, weight: .medium))
                Text(title)
                    .font(.subheadline.weight(.medium))
                    .lineLimit(1)
                    .minimumScaleFactor(0.85)
                    .allowsTightening(true)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(selectionBackground)
            .foregroundStyle(isSelected ? .primary : .secondary)
        }
        .buttonStyle(.plain)
    }

    @ViewBuilder
    private var selectionBackground: some View {
        Capsule()
            .raisedGlassStyle(
                cornerRadius: 999,
                tint: isSelected ? .accentColor : nil,
                colorBoost: isSelected ? 0.16 : 0.06,
                borderOpacity: isSelected ? 0.5 : 0.2,
                interactive: isSelected,
                lift: 3
            )
    }
}

// MARK: - Glass Tab Bar
private struct GlassTabBarStyle: ViewModifier {
    func body(content: Content) -> some View {
        if #available(iOS 26.0, *) {
            content
                .toolbarBackground(.ultraThinMaterial, for: .tabBar)
                .toolbarBackground(.visible, for: .tabBar)
                .tint(.accentColor)
        } else {
            content
        }
    }
}
