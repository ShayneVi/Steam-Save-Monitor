# Steam Backup Manager

<div align="center">

**Comprehensive game save backup and achievement tracking solution with intelligent monitoring**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows-blue.svg)](https://www.microsoft.com/windows)
[![Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB.svg)](https://tauri.app/)

</div>

---

## 📋 Overview

Steam Backup Manager is a powerful desktop application that combines automatic game save backups with comprehensive achievement tracking. It monitors your gaming sessions, tracks achievement unlocks across multiple sources, and creates intelligent backups of both your saves and achievements—all without manual intervention.

### Core Features

- **🏆 Multi-Source Achievement Tracking**
  - Steam API integration for official achievements
  - Goldberg Emulator support
  - Online-fix achievements detection
  - Steamtools compatibility
  - GSE Saves integration
  - Automatic source detection and scanning
  - Manual achievement management

- **💾 Intelligent Backup System**
  - Automatic game save backups using Ludusavi
  - Achievement backup and restore functionality
  - Export achievements in Steam API format
  - Backup versioning with timestamps
  - Configurable backup locations

- **🎮 Real-Time Game Monitoring**
  - Steam API integration for game detection
  - Process-based monitoring for non-Steam games
  - Achievement unlock detection during gameplay
  - Automatic backup on game closure

- **🔔 Advanced Notification System**
  - Rarity-based achievement notifications
  - In-game overlay with customizable appearance
  - Windows native notifications
  - Achievement unlock animations
  - Customizable notification sounds

- **⚙️ Extensive Customization**
  - Five rarity tiers (Common, Uncommon, Rare, Ultra Rare, Legendary)
  - Per-rarity notification customization
  - Custom colors, fonts, icons, and sounds
  - Adjustable notification position and scaling
  - Glow effects and transparency settings

---

## 🚀 Quick Start

### Prerequisites

1. **Windows 10 or later**
2. **[Ludusavi](https://github.com/mtkennerly/ludusavi/releases)** - Download and extract the latest release
3. **Steam Account** (recommended for full functionality)
   - [Steam Web API Key](https://steamcommunity.com/dev/apikey)
   - Steam64 ID (found in your profile URL)

### Installation

1. Download the latest `.msi` installer from the Releases page
2. Run the installer and follow the setup wizard
3. Launch Steam Backup Manager from the Start Menu or Desktop shortcut

### Initial Configuration

Navigate to the **Settings** tab and configure:

| Field | Description | Example |
|-------|-------------|---------|
| **Steam Web API Key** | Personal API key from Steam | `ABC123XYZ789...` |
| **Steam User ID** | Steam account ID | `your_username` |
| **Steam64 ID** | 64-bit Steam ID | `76561198012345678` |
| **Ludusavi Path** | Path to ludusavi.exe | `C:\Tools\Ludusavi\ludusavi.exe` |
| **Backup Directory** | Backup storage location | `C:\GameBackups` |

Click **Save Configuration** to persist your settings.

---

## 📖 Achievement Tracking

### Supported Sources

The application automatically detects and tracks achievements from:

1. **Steam** - Official Steam achievements via Web API
2. **Goldberg Emulator** - Unlocked achievements from Goldberg
3. **Online-fix** - Achievements from Online-fix releases
4. **Steamtools** - Steamtools achievement data
5. **GSE Saves** - Achievements from GSE save files

### Adding Games

1. Navigate to the **Achievements** tab
2. Search for your game using the Steam search
3. Click **Add** on the desired game
4. The app automatically checks which sources have achievement data
5. Select your preferred source
6. If a backup exists, you'll be prompted to restore it

### Achievement Management

**Viewing Achievements:**
- Click any game card to view its achievements
- See unlock status, timestamps, and descriptions
- View global unlock percentages
- Filter achievements by status

**Manual Editing:**
- Click any achievement to open the editor
- Toggle unlock status
- Set custom unlock time
- Changes sync immediately

**Exporting Achievements:**
- Open a game's achievement list
- Click the **Export** button
- Achievements are saved in Steam API format to:
  `Documents/Steam Backup Monitor/{Game_Name}.json`
- Exports include all unlocked achievements with timestamps

**Restoring from Backup:**
- When adding a game with an existing backup
- Confirm the restore prompt
- All backed-up achievements are restored with original timestamps
- The app continues monitoring the selected source for new unlocks

### Real-Time Detection

When you unlock achievements during gameplay:
- Achievements are detected automatically
- Notifications appear via the overlay system
- Database is updated in real-time
- Unlock times are recorded accurately

---

## 🎨 Notification Customization

### Rarity System

Achievements are categorized by global unlock percentage:

| Rarity | Unlock Rate | Default Color |
|--------|-------------|---------------|
| **Common** | 30%+ | Gray |
| **Uncommon** | 20-29% | Green |
| **Rare** | 13-19% | Blue |
| **Ultra Rare** | 5-12% | Purple |
| **Legendary** | 0-4% | Gold |

### Per-Rarity Customization

Navigate to **Achievement Customization** to configure each rarity tier:

**Visual Settings:**
- Border and background colors
- Background opacity (0-100%)
- Title and description text colors
- Glow effects with custom colors
- Notification position (5 presets)
- Scaling (40% - 160%)

**Content Customization:**
- Custom icons (emoji or image files: PNG, JPG, GIF, WEBP, ICO, BMP, SVG)
- Animated GIF support
- Custom fonts (TTF, OTF, WOFF, WOFF2)
- Custom sounds (MP3, WAV, OGG, FLAC, AAC)

**Notification Behavior:**
- Rarities disabled: Windows notification sound plays
- Rarities enabled: Only custom sounds play (if configured)
- No custom sound: Silent notifications
- Individual test buttons per rarity

### Overlay System

Notifications appear as an overlay during gameplay:
- Transparent window over games
- Customizable position and scaling
- Rarity-specific styling
- Progress bar and unlock percentage display
- Automatic fade-in/fade-out animations

---

## 💾 Backup Management

### Automatic Backups

**Game Saves:**
- Triggered automatically when a game closes
- Uses Ludusavi's comprehensive game database
- Stores in configured backup directory
- Includes file count and size information

**Achievements:**
- Manual export via Achievement tab
- Stored in Documents/Steam Backup Monitor
- Steam API compatible JSON format
- Includes unlock timestamps

### Backup Structure

```
BackupPath/
├── GameName1/
│   └── 2025-10-17T10-30-45/
│       └── save files...
└── Documents/Steam Backup Monitor/
    ├── GameName1.json
    └── GameName2.json
```

### Restore Process

1. Add a game to tracking
2. Select achievement source
3. If backup detected, confirm restore
4. Achievements import with original timestamps
5. App monitors for new achievements

---

## 🎯 Advanced Features

### Game Monitoring

**Steam API Monitoring:**
- Polls Steam API every 5 seconds
- Detects game launches and closures
- Automatic backup on session end
- Achievement sync with Steam

**Process Monitoring:**
- Monitor specific game executables
- Support for non-Steam games
- Prevents duplicate backups
- Configurable per-game

### Achievement Sources Detection

When adding a game:
1. App scans all configured achievement sources
2. Displays available sources with achievement counts
3. Select preferred source
4. Scan completes with notification
5. Achievements appear in database

### Global Unlock Percentages

- Fetched from Steam Web API
- Updated when scanning achievements
- Used for rarity calculation
- Displayed in achievement lists
- Powers notification customization

---

## ⚙️ Configuration

### Settings Tab

**Steam Integration:**
- API key for game detection
- User credentials for API access
- 64-bit Steam ID for user identification

**Backup Configuration:**
- Ludusavi executable path
- Backup storage directory
- Test connection functionality

**Application Settings:**
- Auto-start with Windows
- Notification preferences
- Tray icon behavior

### Achievement Customization Tab

**Global Settings:**
- Enable/disable rarity system
- Default notification duration
- Test notification button

**Per-Rarity Configuration:**
- All visual and audio settings
- Individual test buttons
- Real-time preview

---

## 🛠️ Development

### Technology Stack

**Frontend:**
- React 18 with TypeScript
- Tailwind CSS for styling
- Vite for build tooling

**Backend:**
- Rust for performance and safety
- Tauri for desktop integration
- SQLite for achievement storage
- Windows API for notifications

**External Integrations:**
- Steam Web API
- Ludusavi for backup operations
- Multiple achievement source parsers

### Building from Source

#### Prerequisites
- Node.js v16 or later
- Rust (latest stable)
- Tauri prerequisites for Windows

#### Build Steps

```bash
# Clone repository
git clone https://github.com/yourusername/steam-backup-manager.git
cd steam-backup-manager

# Install dependencies
npm install

# Development mode
npm run tauri:dev

# Production build
npm run tauri:build
```

The installer will be in `src-tauri/target/release/bundle/msi/`.

### Project Structure

```
steam-backup-manager/
├── src/                          # React frontend
│   ├── App.tsx                  # Main application
│   ├── components/
│   │   ├── Overlay.tsx          # In-game overlay
│   │   ├── AchievementToast.tsx # Toast notifications
│   │   └── RarityCustomizer.tsx # Rarity settings UI
│   └── types/
│       └── rarityTypes.ts       # Type definitions
├── src-tauri/                    # Rust backend
│   └── src/
│       ├── main.rs              # Application entry
│       ├── achievements.rs      # Database operations
│       ├── achievement_scanner.rs # Multi-source scanning
│       ├── achievement_watcher.rs # Real-time detection
│       ├── steam_achievements.rs # Steam API client
│       ├── steam_monitor.rs     # Game detection
│       ├── ludusavi.rs          # Backup integration
│       ├── notifications.rs     # Notification system
│       ├── overlay.rs           # Overlay management
│       └── config.rs            # Configuration handling
├── public/
│   └── overlay.html             # Overlay window
└── package.json
```

---

## 🐛 Troubleshooting

### Common Issues

**"No achievements found"**
- Verify the game has achievements on Steam
- Check if your selected source has achievement data
- Ensure Steam API credentials are correct
- Try a different achievement source

**"Backup restore failed"**
- Verify backup file exists in Documents/Steam Backup Monitor
- Check JSON file format is valid
- Ensure game was added and scanned first
- Try manually editing the backup file

**"Notifications not showing"**
- Check Windows notification permissions
- Verify overlay window permissions
- Enable rarities in Achievement Customization
- Check custom sound file paths

**"Achievement unlock not detected"**
- Ensure game is being monitored
- Verify achievement source is correct
- Check if source files are accessible
- Restart the application

**"Export failed"**
- Check Documents folder permissions
- Verify Steam Backup Monitor folder exists
- Ensure game name doesn't contain invalid characters
- Check disk space availability

### Debug Mode

For troubleshooting:
1. Run from terminal: `npm run tauri:dev`
2. Check console output for errors
3. Overlay logs appear in terminal
4. Achievement scan results are logged

---

## 🤝 Contributing

Contributions are welcome! Areas for improvement:

- Additional achievement source support
- More notification customization options
- Cloud backup integration
- Achievement statistics and analytics
- Multi-language support

### Contribution Process

1. Fork the repository
2. Create feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit changes (`git commit -m 'Add AmazingFeature'`)
4. Push to branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- **[Ludusavi](https://github.com/mtkennerly/ludusavi)** - Excellent save backup tool that powers this application
- **[Tauri](https://tauri.app/)** - Modern desktop application framework
- **[Steam Web API](https://steamcommunity.com/dev)** - Game and achievement data
- **Achievement Unlocker Community** - Inspiration and source format documentation

---

## 📞 Support

For questions, bug reports, or feature requests:
- Open an issue on GitHub
- Include configuration details (redact sensitive info)
- Provide error messages and steps to reproduce
- Attach relevant screenshots if applicable

---

<div align="center">

**Made with ❤️ for gamers who value their progress and achievements**

[Report Bug](../../issues) · [Request Feature](../../issues)

</div>
