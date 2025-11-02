import React, { useEffect, useState, useRef } from 'react';
import { listen, emit } from '@tauri-apps/api/event';
import { convertFileSrc, invoke } from '@tauri-apps/api/tauri';
import { Trophy, CheckCircle, AlertCircle, GamepadIcon, Save } from 'lucide-react';
import { RaritySettings, defaultRaritySettings, calculateRarity, formatPercentage } from '../types/rarityTypes';

interface NotificationData {
  type: string;
  title?: string;
  body?: string;
  game_name?: string;
  achievement_name?: string;
  achievement_description?: string;
  icon_url?: string;
  files_count?: number;
  total_size?: string;
  error?: string;
  global_unlock_percentage?: number;
  duration_seconds?: number;
}

interface OverlayNotification extends NotificationData {
  id: number;
  visible: boolean;
  customFontFamily?: string; // Loaded custom font family name
  customIconUrl?: string; // Blob URL for custom icon image
}

function Overlay() {
  const [notifications, setNotifications] = useState<OverlayNotification[]>([]);
  const notificationIdCounter = useRef(0);
  const queueRef = useRef<NotificationData[]>([]);
  const processingRef = useRef(false);

  const [raritySettings, setRaritySettings] = useState<RaritySettings>(defaultRaritySettings);
  const raritySettingsRef = useRef<RaritySettings>(defaultRaritySettings);

  // Track loaded fonts to avoid reloading
  const loadedFontsRef = useRef<Map<string, string>>(new Map()); // path -> font-family name

  // Update ref whenever state changes
  useEffect(() => {
    raritySettingsRef.current = raritySettings;
  }, [raritySettings]);

  // Load rarity settings on mount
  useEffect(() => {
    const loadSettings = () => {
      try {
        const saved = localStorage.getItem('raritySettings');
        invoke('debug_log', { message: '=== OVERLAY INITIALIZING ===' });
        invoke('debug_log', { message: `localStorage raritySettings: ${saved ? 'found' : 'not found'}` });

        if (saved) {
          const settings = JSON.parse(saved);
          invoke('debug_log', { message: `Rarities enabled: ${settings.enabled}` });
          if (settings.enabled) {
            invoke('debug_log', { message: 'Custom sounds configured:' });
            for (const rarity of ['Common', 'Uncommon', 'Rare', 'Ultra Rare', 'Legendary']) {
              const soundPath = settings[rarity]?.soundPath;
              if (soundPath) {
                invoke('debug_log', { message: `  ${rarity}: ${soundPath}` });
              }
            }
          }
          setRaritySettings(settings);
        } else {
          invoke('debug_log', { message: 'No rarity settings found in localStorage, using defaults (enabled: false)' });
        }
      } catch (error) {
        invoke('debug_log', { message: `Failed to load rarity settings: ${error}` });
      }
    };

    // Load settings immediately
    loadSettings();
  }, []);

  useEffect(() => {
    // Listen for show-notification events from Rust backend
    const unlistenNotification = listen<[string, NotificationData]>('show-notification', (event) => {
      const [notificationType, data] = event.payload;
      console.log('[Overlay] Received notification:', notificationType, data);

      // Add to queue (duration is now included in notification data)
      queueRef.current.push({
        type: notificationType,
        ...data,
      });

      // Process queue
      processQueue();
    });

    // Listen for rarity settings sync from main window via Tauri events
    const unlistenRaritySync = listen<RaritySettings>('rarity-settings-sync', (event) => {
      const settings = event.payload;
      invoke('debug_log', { message: '=== RARITY SETTINGS SYNCED FROM MAIN WINDOW ===' });
      invoke('debug_log', { message: `Rarities enabled: ${settings.enabled}` });
      if (settings.enabled) {
        invoke('debug_log', { message: 'Custom sounds configured:' });
        for (const rarity of ['Common', 'Uncommon', 'Rare', 'Ultra Rare', 'Legendary']) {
          const soundPath = (settings as any)[rarity]?.soundPath;
          if (soundPath) {
            invoke('debug_log', { message: `  ${rarity}: ${soundPath}` });
          }
        }
      }
      setRaritySettings(settings);
      // Also save to local storage for persistence
      localStorage.setItem('raritySettings', JSON.stringify(settings));
    });

    return () => {
      unlistenNotification.then(fn => fn());
      unlistenRaritySync.then(fn => fn());
    };
  }, []);

  // Helper function to load a custom icon image
  const loadCustomIcon = async (iconPath: string): Promise<string | null> => {
    try {
      // Check if icon is an emoji or file path
      // Simple check: if it contains path separators or has an image extension, it's a file
      const isFilePath = iconPath.includes('\\') || iconPath.includes('/') ||
                        /\.(png|jpg|jpeg|gif|bmp|svg|ico|webp)$/i.test(iconPath);

      if (!isFilePath) {
        // It's an emoji or text, return null to use text rendering
        return null;
      }

      invoke('debug_log', { message: `Loading custom icon: ${iconPath}` });

      // Read icon file via Tauri command
      const bytes = await invoke<number[]>('read_audio_file', { filePath: iconPath });
      invoke('debug_log', { message: `Read ${bytes.length} bytes from icon file` });

      // Convert bytes to Uint8Array
      const uint8Array = new Uint8Array(bytes);

      // Detect MIME type from file extension
      let mimeType = 'image/png'; // default
      const ext = iconPath.toLowerCase().split('.').pop();
      if (ext === 'jpg' || ext === 'jpeg') mimeType = 'image/jpeg';
      else if (ext === 'gif') mimeType = 'image/gif';
      else if (ext === 'bmp') mimeType = 'image/bmp';
      else if (ext === 'svg') mimeType = 'image/svg+xml';
      else if (ext === 'ico') mimeType = 'image/x-icon';
      else if (ext === 'webp') mimeType = 'image/webp';

      // Create blob from bytes
      const blob = new Blob([uint8Array], { type: mimeType });
      const blobUrl = URL.createObjectURL(blob);

      invoke('debug_log', { message: `✓ Icon loaded successfully: ${blobUrl}` });
      invoke('debug_log', { message: `MIME type: ${mimeType}` });

      return blobUrl;
    } catch (error) {
      invoke('debug_log', { message: `✗ Failed to load icon: ${error}` });
      return null;
    }
  };

  // Helper function to load a custom font
  const loadCustomFont = async (fontPath: string, rarity: string): Promise<string | null> => {
    try {
      // Check if already loaded
      if (loadedFontsRef.current.has(fontPath)) {
        const fontFamily = loadedFontsRef.current.get(fontPath)!;
        invoke('debug_log', { message: `Font already loaded: ${fontFamily}` });
        return fontFamily;
      }

      invoke('debug_log', { message: `Loading custom font: ${fontPath}` });

      // Read font file via Tauri command
      const bytes = await invoke<number[]>('read_audio_file', { filePath: fontPath });
      invoke('debug_log', { message: `Read ${bytes.length} bytes from font file` });

      // Convert bytes to Uint8Array
      const uint8Array = new Uint8Array(bytes);

      // Detect font format from file extension
      let fontFormat = 'truetype'; // default for TTF
      const ext = fontPath.toLowerCase().split('.').pop();
      if (ext === 'otf') fontFormat = 'opentype';
      else if (ext === 'woff') fontFormat = 'woff';
      else if (ext === 'woff2') fontFormat = 'woff2';

      // Create blob from bytes
      const blob = new Blob([uint8Array]);
      const blobUrl = URL.createObjectURL(blob);

      // Generate unique font family name
      const fontFamily = `CustomFont-${rarity}-${Date.now()}`;

      // Create @font-face rule
      const fontFace = new FontFace(fontFamily, `url(${blobUrl})`, {
        style: 'normal',
        weight: '400',
      });

      // Load the font
      await fontFace.load();

      // Add to document fonts
      (document as any).fonts.add(fontFace);

      invoke('debug_log', { message: `✓ Font loaded successfully: ${fontFamily}` });

      // Cache the loaded font
      loadedFontsRef.current.set(fontPath, fontFamily);

      return fontFamily;
    } catch (error) {
      invoke('debug_log', { message: `✗ Failed to load font: ${error}` });
      return null;
    }
  };

  const processQueue = async () => {
    // If already processing or queue is empty, return
    if (processingRef.current || queueRef.current.length === 0) {
      return;
    }

    processingRef.current = true;

    const notificationData = queueRef.current.shift()!;
    const newNotification: OverlayNotification = {
      ...notificationData,
      id: notificationIdCounter.current++,
      visible: false,
    };

    // Load custom font and icon if needed (for achievement notifications)
    if (notificationData.type === 'achievement') {
      const currentRaritySettings = raritySettingsRef.current;

      if (currentRaritySettings.enabled && notificationData.global_unlock_percentage !== undefined) {
        const rarity = calculateRarity(notificationData.global_unlock_percentage);
        const fontPath = currentRaritySettings[rarity].fontPath;
        const iconPath = currentRaritySettings[rarity].icon;

        // Load custom font
        if (fontPath) {
          const fontFamily = await loadCustomFont(fontPath, rarity);
          if (fontFamily) {
            newNotification.customFontFamily = fontFamily;
          }
        }

        // Load custom icon
        if (iconPath) {
          const iconUrl = await loadCustomIcon(iconPath);
          if (iconUrl) {
            newNotification.customIconUrl = iconUrl;
          }
        }
      }
    }

    // Play sound based on notification type and rarity settings
    if (notificationData.type === 'achievement') {
      // Use ref to get latest rarity settings (avoids closure issue)
      const currentRaritySettings = raritySettingsRef.current;

      invoke('debug_log', { message: '=== ACHIEVEMENT NOTIFICATION RECEIVED ===' });
      invoke('debug_log', { message: `Rarity enabled: ${currentRaritySettings.enabled}` });
      invoke('debug_log', { message: `Global unlock %: ${notificationData.global_unlock_percentage}` });

      if (currentRaritySettings.enabled && notificationData.global_unlock_percentage !== undefined) {
        // Rarities enabled: ONLY play custom sound if configured
        const rarity = calculateRarity(notificationData.global_unlock_percentage);
        const customSoundPath = currentRaritySettings[rarity].soundPath;

        invoke('debug_log', { message: `Rarity: ${rarity}` });
        invoke('debug_log', { message: `Custom sound path: ${customSoundPath}` });

        if (customSoundPath) {
          // Read audio file via Tauri command and create blob URL
          invoke<number[]>('read_audio_file', { filePath: customSoundPath })
            .then((bytes) => {
              invoke('debug_log', { message: `Read ${bytes.length} bytes from audio file` });

              // Convert bytes to Uint8Array
              const uint8Array = new Uint8Array(bytes);

              // Detect MIME type from file extension
              let mimeType = 'audio/mpeg'; // default for MP3
              const ext = customSoundPath.toLowerCase().split('.').pop();
              if (ext === 'wav') mimeType = 'audio/wav';
              else if (ext === 'ogg') mimeType = 'audio/ogg';
              else if (ext === 'flac') mimeType = 'audio/flac';
              else if (ext === 'aac') mimeType = 'audio/aac';

              // Create blob from bytes
              const blob = new Blob([uint8Array], { type: mimeType });
              const blobUrl = URL.createObjectURL(blob);

              invoke('debug_log', { message: `Created blob URL: ${blobUrl}` });
              invoke('debug_log', { message: `MIME type: ${mimeType}` });

              // Play audio
              const audio = new Audio(blobUrl);
              audio.volume = 1.0;

              audio.play()
                .then(() => {
                  invoke('debug_log', { message: '✓ Custom sound played successfully' });
                  // Clean up blob URL after playing
                  audio.onended = () => URL.revokeObjectURL(blobUrl);
                })
                .catch((error) => {
                  invoke('debug_log', { message: `✗ Failed to play custom sound: ${error.toString()}` });
                  URL.revokeObjectURL(blobUrl);
                });
            })
            .catch((error) => {
              invoke('debug_log', { message: `✗ Failed to read audio file: ${error}` });
            });
        } else {
          invoke('debug_log', { message: 'No custom sound configured - notification will be silent' });
        }
      } else {
        // Rarities disabled: play Windows sound
        invoke('debug_log', { message: 'Playing Windows notification sound (rarities disabled)' });
        invoke('play_windows_notification_sound').catch((error) => {
          invoke('debug_log', { message: `✗ Failed to play Windows sound: ${error}` });
        });
      }
    }

    // Add notification
    setNotifications(prev => [...prev, newNotification]);

    // Trigger entrance animation
    setTimeout(() => {
      setNotifications(prev =>
        prev.map(n => (n.id === newNotification.id ? { ...n, visible: true } : n))
      );
    }, 50);

    // Remove notification after duration (use duration from notification payload, default to 6 seconds for achievements)
    const duration = notificationData.type === 'achievement'
      ? (notificationData.duration_seconds || 6) * 1000
      : 3000;
    setTimeout(() => {
      // Trigger exit animation
      setNotifications(prev =>
        prev.map(n => (n.id === newNotification.id ? { ...n, visible: false } : n))
      );

      // Remove from DOM after animation
      setTimeout(() => {
        setNotifications(prev => {
          const remaining = prev.filter(n => n.id !== newNotification.id);

          // If no notifications left, emit event to hide overlay
          if (remaining.length === 0 && queueRef.current.length === 0) {
            console.log('[Overlay] All notifications done, hiding overlay');
            emit('overlay-notifications-done').catch(err => {
              console.error('[Overlay] Failed to emit overlay-notifications-done:', err);
            });
          }

          return remaining;
        });

        processingRef.current = false;

        // Process next in queue
        if (queueRef.current.length > 0) {
          setTimeout(() => processQueue(), 500);
        }
      }, 300);
    }, duration);
  };

  return (
    <div className="fixed inset-0 pointer-events-none">
      {notifications.map(notification => (
        <OverlayNotificationCard key={notification.id} notification={notification} duration={notification.duration_seconds || 6} raritySettings={raritySettings} />
      ))}
    </div>
  );
}

function OverlayNotificationCard({ notification, duration, raritySettings }: { notification: OverlayNotification; duration: number; raritySettings: RaritySettings }) {
  // Get position and scaling for achievement notifications
  let position = 'top-right';
  let scaling = 1;

  if (notification.type === 'achievement' && raritySettings.enabled && notification.global_unlock_percentage !== undefined) {
    const rarity = calculateRarity(notification.global_unlock_percentage);
    position = raritySettings[rarity].position;
    scaling = raritySettings[rarity].scaling / 100;
  }

  // Position classes mapping
  const positionClasses: Record<string, string> = {
    'top-left': 'top-8 left-8',
    'top-right': 'top-8 right-8',
    'bottom-left': 'bottom-8 left-8',
    'bottom-right': 'bottom-8 right-8',
    'center': 'top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
  };

  const positionClass = positionClasses[position] || positionClasses['top-right'];

  // Determine entrance animation direction based on position
  let entranceAnimation = 'translate-x-full opacity-0'; // default: slide from right
  if (position === 'top-left' || position === 'bottom-left') {
    entranceAnimation = '-translate-x-full opacity-0'; // slide from left
  } else if (position === 'center') {
    entranceAnimation = 'scale-50 opacity-0'; // scale up from center
  }

  const getNotificationContent = () => {
    switch (notification.type) {
      case 'achievement':
        return (
          <AchievementNotification
            gameName={notification.game_name || ''}
            achievementName={notification.achievement_name || ''}
            description={notification.achievement_description || ''}
            iconUrl={notification.icon_url}
            duration={duration}
            globalUnlockPercentage={notification.global_unlock_percentage}
            raritySettings={raritySettings}
            customFontFamily={notification.customFontFamily}
            customIconUrl={notification.customIconUrl}
          />
        );

      case 'game-detected':
        return (
          <GameDetectedNotification gameName={notification.game_name || ''} />
        );

      case 'game-ended':
        return (
          <GameEndedNotification gameName={notification.game_name || ''} />
        );

      case 'backup-success':
        return (
          <BackupSuccessNotification
            gameName={notification.game_name || ''}
            filesCount={notification.files_count || 0}
            totalSize={notification.total_size || ''}
          />
        );

      case 'backup-failed':
        return (
          <BackupFailedNotification
            gameName={notification.game_name || ''}
            error={notification.error || ''}
          />
        );

      default:
        return (
          <GenericNotification
            title={notification.title || 'Notification'}
            body={notification.body || ''}
          />
        );
    }
  };

  return (
    <div
      className={`fixed ${positionClass} pointer-events-auto transform transition-all duration-300 ${
        notification.visible
          ? 'translate-x-0 translate-y-0 scale-100 opacity-100'
          : entranceAnimation
      }`}
      style={{
        willChange: 'transform, opacity',
        transform: notification.visible ? `scale(${scaling})` : undefined,
      }}
    >
      {getNotificationContent()}
    </div>
  );
}

// Achievement notification component
function AchievementNotification({
  gameName,
  achievementName,
  description,
  iconUrl,
  duration,
  globalUnlockPercentage,
  raritySettings,
  customFontFamily,
  customIconUrl,
}: {
  gameName: string;
  achievementName: string;
  description: string;
  iconUrl?: string;
  duration: number;
  globalUnlockPercentage?: number;
  raritySettings: RaritySettings;
  customFontFamily?: string;
  customIconUrl?: string;
}) {
  const [progressWidth, setProgressWidth] = useState(0);

  useEffect(() => {
    // Start progress animation after component mounts
    const timer = setTimeout(() => {
      setProgressWidth(100);
    }, 50);

    return () => clearTimeout(timer);
  }, []);

  // Calculate rarity if enabled
  const rarity = calculateRarity(globalUnlockPercentage);
  const isRarityEnabled = raritySettings.enabled;
  const rarityConfig = raritySettings[rarity];

  // Get rarity-specific or default styling
  const borderColor = isRarityEnabled ? rarityConfig.borderColor : '#F59E0B';
  const backgroundColor = isRarityEnabled ? rarityConfig.backgroundColor : '#1a1f3a';
  const backgroundOpacity = isRarityEnabled ? rarityConfig.backgroundOpacity / 100 : 0.9;
  const titleColor = isRarityEnabled ? rarityConfig.titleColor : '#FDE68A';
  const descriptionColor = isRarityEnabled ? rarityConfig.descriptionColor : '#D1D5DB';
  const glowEffect = isRarityEnabled && rarityConfig.glowEffect;
  const glowColor = isRarityEnabled ? rarityConfig.glowColor : borderColor;
  const icon = isRarityEnabled ? rarityConfig.icon : '🏆';

  // Apply custom font if available
  const fontFamily = customFontFamily || undefined;

  // Determine if we should render icon as image or text/emoji
  const renderIconAsImage = !!customIconUrl;

  return (
    <div
      className="rounded-xl border-2 shadow-2xl backdrop-blur-sm p-6 min-w-[400px] max-w-[500px] relative"
      style={{
        backgroundColor: `${backgroundColor}${Math.round(backgroundOpacity * 255).toString(16).padStart(2, '0')}`,
        borderColor: borderColor,
        boxShadow: glowEffect ? `0 0 30px ${glowColor}, 0 0 60px ${glowColor}40` : undefined,
        fontFamily: fontFamily,
      }}
    >
      {/* Glow effect */}
      {glowEffect && (
        <div
          className="absolute inset-0 rounded-xl blur-xl"
          style={{ background: `radial-gradient(circle at 50% 50%, ${glowColor}40 0%, transparent 70%)` }}
        ></div>
      )}

      {/* Header */}
      <div className="relative flex items-center gap-3 mb-4">
        {/* Render icon as image or text/emoji */}
        {renderIconAsImage ? (
          <img
            src={customIconUrl}
            alt="Achievement Icon"
            className="object-contain"
            style={{
              width: '48px',
              height: '48px',
              maxWidth: '48px',
              maxHeight: '48px',
            }}
          />
        ) : (
          <span className="text-3xl">{icon}</span>
        )}
        <div className="flex-1 flex items-center gap-2">
          <span
            className="font-bold text-lg tracking-wide"
            style={{ color: titleColor }}
          >
            ACHIEVEMENT UNLOCKED
          </span>
          {globalUnlockPercentage !== undefined && (
            <span className="text-sm opacity-75" style={{ color: titleColor }}>
              ({formatPercentage(globalUnlockPercentage)})
            </span>
          )}
          {isRarityEnabled && (
            <span
              className="ml-auto px-2 py-0.5 rounded text-xs font-bold"
              style={{
                backgroundColor: `${borderColor}30`,
                color: borderColor,
                borderColor: borderColor,
                border: '1px solid',
              }}
            >
              {rarity}
            </span>
          )}
        </div>
      </div>

      {/* Content */}
      <div className="relative flex gap-4">
        {iconUrl && (
          <div className="flex-shrink-0">
            <img
              src={iconUrl}
              alt={achievementName}
              className="w-20 h-20 rounded-lg border-2"
              style={{ borderColor: borderColor + '30' }}
            />
          </div>
        )}

        <div className="flex-1 min-w-0">
          <h3 className="font-bold text-xl mb-2" style={{ color: titleColor }}>
            {achievementName}
          </h3>
          <p className="text-sm mb-3" style={{ color: descriptionColor }}>
            {description}
          </p>
          <p className="text-xs" style={{ color: descriptionColor, opacity: 0.8 }}>
            {gameName}
          </p>
        </div>
      </div>

      {/* Progress bar animation */}
      <div className="relative mt-4 h-1 rounded-full overflow-hidden" style={{ backgroundColor: `${borderColor}30` }}>
        <div
          className="h-full rounded-full"
          style={{
            background: `linear-gradient(90deg, ${borderColor} 0%, ${borderColor}CC 100%)`,
            width: `${progressWidth}%`,
            transition: `width ${duration}s linear`
          }}
        ></div>
      </div>
    </div>
  );
}

// Game detected notification
function GameDetectedNotification({ gameName }: { gameName: string }) {
  return (
    <div className="bg-gradient-to-br from-[#1a1f3a] to-[#2a2f4a] rounded-xl border-2 border-blue-500/50 shadow-2xl backdrop-blur-sm p-5 min-w-[350px]">
      <div className="flex items-center gap-3">
        <div className="p-2 bg-blue-500/20 rounded-lg">
          <GamepadIcon className="w-5 h-5 text-blue-400" />
        </div>
        <div className="flex-1">
          <p className="text-blue-300 font-semibold text-sm">Game Detected</p>
          <p className="text-white font-medium">{gameName}</p>
        </div>
      </div>
    </div>
  );
}

// Game ended notification
function GameEndedNotification({ gameName }: { gameName: string }) {
  return (
    <div className="bg-gradient-to-br from-[#1a1f3a] to-[#2a2f4a] rounded-xl border-2 border-purple-500/50 shadow-2xl backdrop-blur-sm p-5 min-w-[350px]">
      <div className="flex items-center gap-3">
        <div className="p-2 bg-purple-500/20 rounded-lg">
          <GamepadIcon className="w-5 h-5 text-purple-400" />
        </div>
        <div className="flex-1">
          <p className="text-purple-300 font-semibold text-sm">Game Ended</p>
          <p className="text-white font-medium">{gameName}</p>
        </div>
      </div>
    </div>
  );
}

// Backup success notification
function BackupSuccessNotification({
  gameName,
  filesCount,
  totalSize,
}: {
  gameName: string;
  filesCount: number;
  totalSize: string;
}) {
  return (
    <div className="bg-gradient-to-br from-[#1a1f3a] to-[#2a2f4a] rounded-xl border-2 border-emerald-500/50 shadow-2xl backdrop-blur-sm p-5 min-w-[350px]">
      <div className="flex items-start gap-3">
        <div className="p-2 bg-emerald-500/20 rounded-lg">
          <CheckCircle className="w-5 h-5 text-emerald-400" />
        </div>
        <div className="flex-1">
          <p className="text-emerald-300 font-semibold text-sm">Backup Complete</p>
          <p className="text-white font-medium mb-1">{gameName}</p>
          <p className="text-gray-400 text-xs">
            {filesCount} files • {totalSize}
          </p>
        </div>
      </div>
    </div>
  );
}

// Backup failed notification
function BackupFailedNotification({ gameName, error }: { gameName: string; error: string }) {
  return (
    <div className="bg-gradient-to-br from-[#1a1f3a] to-[#2a2f4a] rounded-xl border-2 border-red-500/50 shadow-2xl backdrop-blur-sm p-5 min-w-[350px] max-w-[450px]">
      <div className="flex items-start gap-3">
        <div className="p-2 bg-red-500/20 rounded-lg">
          <AlertCircle className="w-5 h-5 text-red-400" />
        </div>
        <div className="flex-1">
          <p className="text-red-300 font-semibold text-sm">Backup Failed</p>
          <p className="text-white font-medium mb-1">{gameName}</p>
          <p className="text-gray-400 text-xs">{error}</p>
        </div>
      </div>
    </div>
  );
}

// Generic notification
function GenericNotification({ title, body }: { title: string; body: string }) {
  return (
    <div className="bg-gradient-to-br from-[#1a1f3a] to-[#2a2f4a] rounded-xl border-2 border-gray-500/50 shadow-2xl backdrop-blur-sm p-5 min-w-[350px]">
      <div className="flex items-start gap-3">
        <div className="p-2 bg-gray-500/20 rounded-lg">
          <Save className="w-5 h-5 text-gray-400" />
        </div>
        <div className="flex-1">
          <p className="text-gray-300 font-semibold text-sm">{title}</p>
          <p className="text-white text-sm">{body}</p>
        </div>
      </div>
    </div>
  );
}

// Note: Progress bar animation duration is now dynamically set via inline styles in AchievementNotification component

export default Overlay;
