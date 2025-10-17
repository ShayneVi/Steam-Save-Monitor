# Steam Save Monitor

<div align="center">

**Automatic game save backup solution with intelligent Steam and process monitoring**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows-blue.svg)](https://www.microsoft.com/windows)
[![Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB.svg)](https://tauri.app/)

</div>

---

## 📋 Overview

Steam Save Monitor is a desktop application that automatically monitors your gaming sessions and creates backups of your game saves using [Ludusavi](https://github.com/mtkennerly/ludusavi). It intelligently detects when games are launched and closed, then automatically backs up your progress without any manual intervention.

### Key Features

- **🎮 Dual Monitoring System**
  - Steam API integration for automatic game detection
  - Process-based monitoring for custom game executables
  - Intelligent coordination between both systems

- **💾 Automatic Backups**
  - Backs up game saves immediately after closing a game
  - Uses Ludusavi's comprehensive game database
  - Configurable backup directory

- **🔔 Smart Notifications**
  - Windows native notifications for backup status
  - System sound alerts for important events
  - Real-time feedback on backup operations

- **⚡ Performance Optimized**
  - Cached manifest for instant game lookup
  - Minimal system resource usage
  - Runs efficiently in the background

- **🎯 User-Friendly Interface**
  - Clean, modern UI with dark theme
  - Easy configuration wizard
  - Searchable game database
  - System tray integration

---

## 🚀 Quick Start

### Prerequisites

Before installing Steam Save Monitor, ensure you have:

1. **Windows 10 or later**
2. **[Ludusavi](https://github.com/mtkennerly/ludusavi/releases)** - Download and extract the latest release
3. **Steam Account** (optional, for Steam API monitoring)
   - [Steam Web API Key](https://steamcommunity.com/dev/apikey)
   - Your Steam64 ID (found in your profile URL)

### Installation

1. Download the latest `.msi` installer from the [Releases](../../releases) page
2. Run the installer and follow the setup wizard
3. Launch Steam Save Monitor from the Start Menu or Desktop shortcut

### Initial Configuration

1. Open the application and navigate to the **Settings** tab
2. Configure the following fields:

   | Field | Description | Example |
   |-------|-------------|---------|
   | **Steam Web API Key** | Your personal API key from Steam | `ABC123XYZ789...` |
   | **Steam User ID** | Your Steam64 ID | `76561198012345678` |
   | **Ludusavi Executable Path** | Path to ludusavi.exe | `C:\Tools\Ludusavi\ludusavi.exe` |
   | **Backup Directory** | Where backups will be stored | `C:\GameBackups` |

3. Click **Test** next to the Ludusavi path to verify it's working
4. Click **Save Configuration**
5. Navigate to the **Game Executables** tab to configure process monitoring (optional)

---

## 📖 How It Works

### Steam API Monitoring

When enabled, the app polls the Steam API every 5 seconds to detect:
- Which game you're currently playing
- When you start playing a game
- When you stop playing a game

When a game session ends, it automatically triggers a backup.

### Process Monitoring

For games not launched through Steam (or for additional precision):
1. Navigate to the **Game Executables** tab
2. Search for your game in the Ludusavi manifest
3. Click **Select EXE** and browse to the game's executable
4. The app will now monitor that specific process

**Smart Coordination:** When a process-monitored game is running, Steam monitoring is automatically paused to prevent duplicate backups.

### Backup Process

1. Game closure is detected
2. Brief delay to ensure all save files are written
3. Ludusavi is called to backup the game's saves
4. Results are displayed via notification
5. Backup is stored in your configured directory

---

## 🎯 Usage Guide

### Managing Game Executables

The **Game Executables** tab allows you to configure process-based monitoring:

- **Search:** Use the search bar to filter through Ludusavi's game database
- **Add Game:** Click "Select EXE" next to a game name and browse to its executable
- **Remove Game:** Click the trash icon next to configured games
- **Refresh Manifest:** Update the game database (cached for 24 hours)

### Notification System

Notifications inform you of:
- ✅ **Backup Success** - Shows number of files backed up and total size
- ▶️ **Game Detected** - When a game starts
- ⏹️ **Game Ended** - When a game closes
- ⚠️ **Errors** - Any issues during backup
- 🔍 **Game Not Found** - When a game isn't in Ludusavi's database

### System Tray

The app minimizes to the system tray:
- **Left Click** - Open the settings window
- **Right Click** - Access tray menu
  - Open Settings
  - Quit

---

## ⚙️ Configuration Options

### Auto Start
Launch Steam Save Monitor automatically when Windows starts.

### Notifications
Enable or disable Windows notifications for backup events.

### Backup Path
Ludusavi stores backups in a date-organized structure:
```
BackupPath/
├── GameName1/
│   └── 2025-10-17T10-30-45/
│       └── save files...
└── GameName2/
    └── 2025-10-17T14-22-10/
        └── save files...
```

---

## 🛠️ Development

### Technology Stack

- **Frontend:** React + TypeScript + Tailwind CSS
- **Backend:** Rust + Tauri
- **APIs:** Steam Web API, Windows Notifications
- **External Tools:** Ludusavi

### Building from Source

#### Prerequisites
- [Node.js](https://nodejs.org/) (v16 or later)
- [Rust](https://www.rust-lang.org/tools/install)
- [Tauri Prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites)

#### Build Steps

```bash
# Clone the repository
git clone https://github.com/yourusername/steam-backup-manager.git
cd steam-backup-manager

# Install dependencies
npm install

# Run in development mode
npm run tauri:dev

# Build for production
npm run tauri:build
```

The production build will be available in `src-tauri/target/release/bundle/`.

### Project Structure

```
steam-backup-manager/
├── src/                    # React frontend
│   ├── App.tsx            # Main application component
│   ├── main.tsx           # Entry point
│   └── index.css          # Global styles
├── src-tauri/             # Rust backend
│   └── src/
│       ├── main.rs        # Tauri application entry
│       ├── config.rs      # Configuration management
│       ├── steam_monitor.rs    # Steam API integration
│       ├── process_monitor.rs  # Process detection
│       ├── ludusavi.rs    # Ludusavi integration
│       └── notifications.rs    # Windows notifications
├── package.json
└── tauri.conf.json
```

---

## 🐛 Troubleshooting

### Common Issues

**"Ludusavi not found"**
- Ensure Ludusavi is installed and the path is correct
- Click "Browse" and navigate to `ludusavi.exe`
- Click "Test" to verify the connection

**"Steam API error"**
- Verify your API key at [steamcommunity.com/dev/apikey](https://steamcommunity.com/dev/apikey)
- Check your Steam64 ID is correct
- Ensure you're logged into Steam

**Notifications not showing**
- Check Windows notification settings
- Ensure notifications are enabled in the app settings
- Verify Windows PowerShell is installed

**Game not detected**
- For Steam games: Ensure Steam API is configured correctly
- For other games: Add the executable in the Game Executables tab
- Check if the game name exists in Ludusavi's manifest

**Backup failed**
- Verify Ludusavi path is correct
- Ensure backup directory has write permissions
- Check if the game is supported by Ludusavi

### Getting Help

For additional support:
1. Check the [Ludusavi documentation](https://github.com/mtkennerly/ludusavi)
2. Open an issue on GitHub with:
   - Your configuration (redact sensitive info)
   - Error messages
   - Steps to reproduce

---

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- [Ludusavi](https://github.com/mtkennerly/ludusavi) - The excellent save backup tool that powers this application
- [Tauri](https://tauri.app/) - For the modern desktop application framework
- [Steam Web API](https://steamcommunity.com/dev) - For game detection capabilities

---

## 📞 Contact

For questions, suggestions, or issues, please open an issue on GitHub.

---

<div align="center">

**Made with ❤️ for gamers who value their progress**

[Report Bug](../../issues) · [Request Feature](../../issues) · [Documentation](../../wiki)

</div>