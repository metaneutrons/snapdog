import SwiftUI

// MARK: - Config Model

@Observable
final class ConfigModel {
    var subsonic = SubsonicSection()
    var zones: [ZoneEntry] = []
    var clients: [ClientEntry] = []
    var radios: [RadioEntry] = []
    var mqtt = MqttSection()
    var airplayPassword = ""
    var codec = "flac"

    struct SubsonicSection: Equatable {
        var url = ""
        var username = ""
        var password = ""
    }

    struct MqttSection: Equatable {
        var enabled = false
        var broker = ""
        var clientId = "snapdog"
        var username = ""
        var password = ""
        var baseTopic = "snapdog"
    }

    struct ZoneEntry: Identifiable {
        let id = UUID()
        var name = ""
        var icon = "🏠"
    }

    struct ClientEntry: Identifiable {
        let id = UUID()
        var name = ""
        var mac = ""
        var zone = ""
        var icon = "🔊"
    }

    struct RadioEntry: Identifiable {
        let id = UUID()
        var name = ""
        var url = ""
    }
}

// MARK: - Config View

struct ConfigView: View {
    @Bindable var serverManager: ServerManager
    @State private var config = ConfigModel()
    @State private var saveTask: Task<Void, Never>?

    var body: some View {
        TabView {
            Tab("Music", systemImage: "music.note.house") {
                Form { musicForm }.formStyle(.grouped)
            }
            Tab("Zones & Clients", systemImage: "hifispeaker.2") {
                Form { zonesClientsForm }.formStyle(.grouped)
            }
            Tab("Advanced", systemImage: "gearshape") {
                Form { advancedForm }.formStyle(.grouped)
            }
        }
        .tabViewStyle(.sidebarAdaptable)
        .frame(minWidth: 520, minHeight: 400)
        .onAppear { load() }
        .onChange(of: config.subsonic) { _, _ in debounceSave() }
        .onChange(of: config.mqtt) { _, _ in debounceSave() }
        .onChange(of: config.airplayPassword) { _, _ in debounceSave() }
        .onChange(of: config.codec) { _, _ in debounceSave() }
        .onChange(of: config.zones.count) { _, _ in debounceSave() }
        .onChange(of: config.clients.count) { _, _ in debounceSave() }
        .onChange(of: config.radios.count) { _, _ in debounceSave() }
    }

    // MARK: - Music Tab

    @ViewBuilder
    private var musicForm: some View {
        SwiftUI.Section("Subsonic / Navidrome") {
            TextField("Server URL", text: $config.subsonic.url, prompt: Text("http://navidrome:4533"))
            TextField("Username", text: $config.subsonic.username)
            SecureField("Password", text: $config.subsonic.password)
        }

        SwiftUI.Section {
            List {
                ForEach($config.radios) { $radio in
                    HStack {
                        TextField("Name", text: $radio.name)
                        TextField("Stream URL", text: $radio.url, prompt: Text("https://..."))
                            .foregroundStyle(.secondary)
                    }
                }
                .onDelete { config.radios.remove(atOffsets: $0) }
                .onMove { config.radios.move(fromOffsets: $0, toOffset: $1) }
            }
            .frame(minHeight: 80)
        } header: {
            Text("Radio Stations")
        } footer: {
            HStack {
                Button("", systemImage: "plus") {
                    config.radios.append(.init())
                }
                Button("", systemImage: "minus") {
                    if !config.radios.isEmpty { config.radios.removeLast() }
                }
                .disabled(config.radios.isEmpty)
                Spacer()
            }
            .buttonStyle(.borderless)
        }
    }

    // MARK: - Zones & Clients Tab

    @ViewBuilder
    private var zonesClientsForm: some View {
        SwiftUI.Section {
            List {
                ForEach($config.zones) { $zone in
                    HStack {
                        TextField("", text: $zone.icon)
                            .frame(width: 36)
                            .multilineTextAlignment(.center)
                        TextField("Zone Name", text: $zone.name)
                    }
                }
                .onDelete { config.zones.remove(atOffsets: $0) }
                .onMove { config.zones.move(fromOffsets: $0, toOffset: $1) }
            }
            .frame(minHeight: 80)
        } header: {
            Text("Zones")
        } footer: {
            HStack {
                Button("", systemImage: "plus") {
                    config.zones.append(.init(name: "New Zone"))
                }
                Button("", systemImage: "minus") {
                    if !config.zones.isEmpty { config.zones.removeLast() }
                }
                .disabled(config.zones.isEmpty)
                Spacer()
            }
            .buttonStyle(.borderless)
        }

        SwiftUI.Section {
            List {
                ForEach($config.clients) { $client in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            TextField("", text: $client.icon)
                                .frame(width: 36)
                                .multilineTextAlignment(.center)
                            TextField("Name", text: $client.name)
                        }
                        HStack {
                            TextField("MAC", text: $client.mac, prompt: Text("aa:bb:cc:dd:ee:ff"))
                            TextField("Zone", text: $client.zone, prompt: Text("Zone name"))
                        }
                        .font(.callout)
                        .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 2)
                }
                .onDelete { config.clients.remove(atOffsets: $0) }
                .onMove { config.clients.move(fromOffsets: $0, toOffset: $1) }
            }
            .frame(minHeight: 100)
        } header: {
            Text("Clients")
        } footer: {
            HStack {
                Button("", systemImage: "plus") {
                    config.clients.append(.init())
                }
                Button("", systemImage: "minus") {
                    if !config.clients.isEmpty { config.clients.removeLast() }
                }
                .disabled(config.clients.isEmpty)
                Spacer()
            }
            .buttonStyle(.borderless)
        }
    }

    // MARK: - Advanced Tab

    @ViewBuilder
    private var advancedForm: some View {
        SwiftUI.Section("Audio") {
            Picker("Streaming Codec", selection: $config.codec) {
                Text("FLAC (lossless)").tag("flac")
                Text("PCM (uncompressed)").tag("pcm")
                Text("F32+LZ4 (low latency)").tag("f32lz4")
                Text("F32+LZ4 encrypted").tag("f32lz4e")
            }
            .pickerStyle(.menu)
        }

        SwiftUI.Section("AirPlay") {
            SecureField("Password", text: $config.airplayPassword, prompt: Text("No password"))
                .help("Optional password for AirPlay connections")
        }

        SwiftUI.Section("MQTT") {
            Toggle("Enable MQTT", isOn: $config.mqtt.enabled)
            Group {
                TextField("Broker", text: $config.mqtt.broker, prompt: Text("host:port"))
                TextField("Client ID", text: $config.mqtt.clientId)
                TextField("Username", text: $config.mqtt.username, prompt: Text("Optional"))
                SecureField("Password", text: $config.mqtt.password)
                TextField("Base Topic", text: $config.mqtt.baseTopic)
            }
            .disabled(!config.mqtt.enabled)
        }
    }

    // MARK: - Auto-save

    private func debounceSave() {
        saveTask?.cancel()
        saveTask = Task {
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled else { return }
            save()
        }
    }

    private func load() {
        serverManager.ensureConfigExists()
        do {
            config = try TOMLConfigParser.load(from: serverManager.configPath)
        } catch {
            config = ConfigModel()
        }
    }

    private func save() {
        try? TOMLConfigParser.save(config, to: serverManager.configPath)
    }
}
