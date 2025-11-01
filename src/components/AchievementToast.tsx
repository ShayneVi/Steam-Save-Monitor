import React, { useEffect, useState, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { convertFileSrc, invoke } from '@tauri-apps/api/tauri';
import { Trophy } from 'lucide-react';
import './AchievementToast.css';
import { RaritySettings, defaultRaritySettings, calculateRarity, formatPercentage, getRarityColor } from '../types/rarityTypes';

interface AchievementUnlockEvent {
  app_id: number;
  game_name: string;
  achievement_id: string;
  display_name: string;
  description: string;
  icon_url?: string;
  unlock_time: number;
  source: string;
  global_unlock_percentage?: number;
}

interface ToastData extends AchievementUnlockEvent {
  id: number;
  visible: boolean;
}

export function AchievementToastContainer() {
  const [toasts, setToasts] = useState<ToastData[]>([]);
  const toastIdCounter = useRef(0);
  const queueRef = useRef<AchievementUnlockEvent[]>([]);
  const processingRef = useRef(false);
  const [raritySettings, setRaritySettings] = useState<RaritySettings>(defaultRaritySettings);

  // Load rarity settings from localStorage
  useEffect(() => {
    const loadRaritySettings = () => {
      const saved = localStorage.getItem('raritySettings');
      if (saved) {
        try {
          setRaritySettings(JSON.parse(saved));
        } catch (error) {
          console.error('Failed to load rarity settings:', error);
        }
      }
    };

    loadRaritySettings();

    // Listen for rarity settings updates
    const handleRaritySettingsUpdate = (event: any) => {
      setRaritySettings(event.detail);
    };

    window.addEventListener('rarity-settings-updated', handleRaritySettingsUpdate);

    return () => {
      window.removeEventListener('rarity-settings-updated', handleRaritySettingsUpdate);
    };
  }, []);

  useEffect(() => {
    // Listen for achievement unlock events from Tauri backend
    const unlisten = listen<AchievementUnlockEvent>('achievement-unlocked', (event) => {
      console.log('🏆 Achievement unlocked event received:', event.payload);

      // Add to queue
      queueRef.current.push(event.payload);

      // Process queue
      processQueue();
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  const processQueue = () => {
    // If already processing or queue is empty, return
    if (processingRef.current || queueRef.current.length === 0) {
      return;
    }

    processingRef.current = true;

    const event = queueRef.current.shift()!;
    const newToast: ToastData = {
      ...event,
      id: toastIdCounter.current++,
      visible: false,
    };

    // Play sound based on rarity settings
    if (raritySettings.enabled && event.global_unlock_percentage !== undefined) {
      // Rarities enabled: ONLY play custom sound if configured
      const rarity = calculateRarity(event.global_unlock_percentage);
      const customSoundPath = raritySettings[rarity].soundPath;

      if (customSoundPath) {
        // Play custom sound
        try {
          // Convert file path to Tauri asset URL
          const assetUrl = convertFileSrc(customSoundPath);
          console.log('Playing custom sound for', rarity, ':', assetUrl);
          console.log('Original path:', customSoundPath);

          const audio = new Audio(assetUrl);
          audio.volume = 1.0;
          audio.play().catch((error) => {
            console.error('Failed to play custom sound:', error);
            console.error('Asset URL:', assetUrl);
          });
        } catch (error) {
          console.error('Failed to create audio element:', error);
        }
      }
      // Note: If no custom sound is set, no sound plays (as per user requirement)
    } else {
      // Rarities disabled: always play Windows sound
      invoke('play_windows_notification_sound').catch((error) => {
        console.error('Failed to play Windows notification sound:', error);
      });
    }

    // Add toast with animation
    setToasts(prev => [...prev, newToast]);

    // Trigger entrance animation
    setTimeout(() => {
      setToasts(prev =>
        prev.map(t => (t.id === newToast.id ? { ...t, visible: true } : t))
      );
    }, 50);

    // Remove toast after 6 seconds
    setTimeout(() => {
      // Trigger exit animation
      setToasts(prev =>
        prev.map(t => (t.id === newToast.id ? { ...t, visible: false } : t))
      );

      // Remove from DOM after animation
      setTimeout(() => {
        setToasts(prev => prev.filter(t => t.id !== newToast.id));
        processingRef.current = false;

        // Process next in queue
        if (queueRef.current.length > 0) {
          setTimeout(() => processQueue(), 500);
        }
      }, 300);
    }, 6000);
  };

  return (
    <div className="achievement-toast-container">
      {toasts.map(toast => (
        <AchievementToast key={toast.id} data={toast} raritySettings={raritySettings} />
      ))}
    </div>
  );
}

function AchievementToast({ data, raritySettings }: { data: ToastData; raritySettings: RaritySettings }) {
  // Calculate rarity based on global unlock percentage
  const rarity = calculateRarity(data.global_unlock_percentage);
  const rarityConfig = raritySettings[rarity];
  const isRarityEnabled = raritySettings.enabled;

  // Get rarity-specific or default styling
  const borderColor = isRarityEnabled ? rarityConfig.borderColor : '#3B82F6';
  const backgroundColor = isRarityEnabled ? rarityConfig.backgroundColor : '#1E3A8A';
  const backgroundOpacity = isRarityEnabled ? rarityConfig.backgroundOpacity / 100 : 0.9;
  const titleColor = isRarityEnabled ? rarityConfig.titleColor : '#DBEAFE';
  const descriptionColor = isRarityEnabled ? rarityConfig.descriptionColor : '#BFDBFE';
  const scaling = isRarityEnabled ? rarityConfig.scaling / 100 : 1;
  const position = isRarityEnabled ? rarityConfig.position : 'top-right';
  const glowEffect = isRarityEnabled && rarityConfig.glowEffect;
  const glowColor = isRarityEnabled ? rarityConfig.glowColor : borderColor;
  const icon = isRarityEnabled ? rarityConfig.icon : '🏆';

  // Position classes
  const positionClasses: Record<string, string> = {
    'top-left': 'top-8 left-8',
    'top-right': 'top-8 right-8',
    'bottom-left': 'bottom-8 left-8',
    'bottom-right': 'bottom-8 right-8',
    'center': 'top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
  };

  const positionClass = positionClasses[position] || positionClasses['top-right'];

  // Dynamic styles
  // Note: Don't set transform here as it will conflict with CSS animations
  // Instead, we'll use a wrapper div for scaling
  const containerStyle: React.CSSProperties = {
    borderColor: borderColor,
    backgroundColor: `${backgroundColor}${Math.round(backgroundOpacity * 255).toString(16).padStart(2, '0')}`,
    boxShadow: glowEffect ? `0 0 30px ${glowColor}, 0 0 60px ${glowColor}40, 0 4px 16px rgba(0, 0, 0, 0.5)` : '0 4px 16px rgba(0, 0, 0, 0.5)',
  };

  const wrapperStyle: React.CSSProperties = {
    transform: `scale(${scaling})`,
  };

  const glowStyle: React.CSSProperties = {
    background: `radial-gradient(circle at 50% 50%, ${glowColor}40 0%, transparent 70%)`,
  };

  return (
    <div className={`fixed ${positionClass}`} style={wrapperStyle}>
      <div
        className={`achievement-toast ${data.visible ? 'visible' : ''}`}
        style={containerStyle}
      >
        {glowEffect && <div className="achievement-toast-glow" style={glowStyle}></div>}

        <div className="achievement-toast-header">
          <span className="text-2xl mr-2">{icon}</span>
          <span className="achievement-toast-title" style={{ color: titleColor }}>
            ACHIEVEMENT UNLOCKED
            {data.global_unlock_percentage !== undefined && (
              <span className="ml-2 text-sm opacity-75">
                ({formatPercentage(data.global_unlock_percentage)})
              </span>
            )}
          </span>
          {isRarityEnabled && (
            <span
              className="ml-2 px-2 py-0.5 rounded text-xs font-bold"
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

        <div className="achievement-toast-content">
          {data.icon_url && (
            <div className="achievement-toast-icon" style={{ borderColor: borderColor }}>
              <img src={data.icon_url} alt={data.display_name} />
            </div>
          )}

          <div className="achievement-toast-details">
            <div className="achievement-toast-name" style={{ color: titleColor }}>
              {data.display_name}
            </div>
            <div className="achievement-toast-description" style={{ color: descriptionColor }}>
              {data.description}
            </div>
            <div className="achievement-toast-game" style={{ color: descriptionColor, opacity: 0.8 }}>
              {data.game_name}
            </div>
          </div>
        </div>

        <div className="achievement-toast-progress-bar" style={{ backgroundColor: `${borderColor}30` }}>
          <div
            className="achievement-toast-progress-fill"
            style={{ backgroundColor: borderColor }}
          ></div>
        </div>
      </div>
    </div>
  );
}
