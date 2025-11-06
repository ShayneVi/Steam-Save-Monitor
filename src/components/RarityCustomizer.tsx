import React, { useState } from 'react';
import { Settings, Upload, X, Volume2, Type } from 'lucide-react';
import { RarityTier, RarityCustomization, NotificationPosition, ScalingOption } from '../types/rarityTypes';
import { invoke } from '@tauri-apps/api/tauri';

interface RarityCustomizerProps {
  rarity: RarityTier;
  settings: RarityCustomization;
  onChange: (settings: RarityCustomization) => void;
  onTest: () => void;
}

const POSITIONS: NotificationPosition[] = ['top-left', 'top-right', 'bottom-left', 'bottom-right', 'center'];
const SCALING_OPTIONS: ScalingOption[] = [40, 60, 80, 100, 120, 140, 160];

export function RarityCustomizer({ rarity, settings, onChange, onTest }: RarityCustomizerProps) {
  const [isExpanded, setIsExpanded] = useState(false);

  const handleFileSelect = async (type: 'sound' | 'font' | 'icon') => {
    try {
      const selected = await invoke<string | null>('browse_file');
      if (selected) {
        if (type === 'sound') {
          onChange({ ...settings, soundPath: selected });
        } else if (type === 'font') {
          onChange({ ...settings, fontPath: selected });
        } else if (type === 'icon') {
          onChange({ ...settings, icon: selected });
        }
      }
    } catch (error) {
      console.error(`Failed to select ${type}:`, error);
    }
  };

  const getRarityColor = () => {
    switch (rarity) {
      case 'Common': return '#9CA3AF';
      case 'Uncommon': return '#10B981';
      case 'Rare': return '#3B82F6';
      case 'Ultra Rare': return '#A855F7';
      case 'Legendary': return '#F59E0B';
    }
  };

  return (
    <div className="bg-[#0f1420] border-2 rounded-xl overflow-hidden" style={{ borderColor: getRarityColor() }}>
      {/* Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full p-5 flex items-center justify-between hover:bg-[#13172a] transition-colors"
      >
        <div className="flex items-center gap-3">
          {/* Only show icon if it's an emoji (not a file path) */}
          {settings.icon && !settings.icon.includes('\\') && !settings.icon.includes('/') && !/\.(png|jpg|jpeg|gif|bmp|svg|ico|webp)$/i.test(settings.icon) && (
            <span className="text-3xl">{settings.icon}</span>
          )}
          <div className="text-left">
            <h3 className="text-xl font-bold" style={{ color: getRarityColor() }}>
              {rarity}
            </h3>
            <p className="text-sm text-gray-400">
              {rarity === 'Common' && '30%+ unlock rate'}
              {rarity === 'Uncommon' && '20-29% unlock rate'}
              {rarity === 'Rare' && '13-19% unlock rate'}
              {rarity === 'Ultra Rare' && '5-12% unlock rate'}
              {rarity === 'Legendary' && '0-4% unlock rate'}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={(e) => {
              e.stopPropagation();
              onTest();
            }}
            className="px-4 py-2 rounded-lg font-semibold transition-all shadow-lg hover:scale-105"
            style={{
              backgroundColor: getRarityColor() + '30',
              borderColor: getRarityColor() + '50',
              color: getRarityColor(),
              border: '2px solid',
            }}
          >
            Test
          </button>
          <span className={`transform transition-transform ${isExpanded ? 'rotate-180' : ''}`}>
            ▼
          </span>
        </div>
      </button>

      {/* Customization Panel */}
      {isExpanded && (
        <div className="p-6 border-t-2 space-y-6" style={{ borderColor: getRarityColor() + '30' }}>
          {/* Colors Row */}
          <div className="grid grid-cols-2 gap-4">
            {/* Border Color */}
            <div>
              <label className="block text-sm font-semibold text-gray-200 mb-2">Border Color</label>
              <input
                type="color"
                value={settings.borderColor}
                onChange={(e) => onChange({ ...settings, borderColor: e.target.value })}
                className="w-full h-12 rounded-lg cursor-pointer border-2 border-[#2a3142]"
              />
            </div>

            {/* Background Color */}
            <div>
              <label className="block text-sm font-semibold text-gray-200 mb-2">Background Color</label>
              <input
                type="color"
                value={settings.backgroundColor}
                onChange={(e) => onChange({ ...settings, backgroundColor: e.target.value })}
                className="w-full h-12 rounded-lg cursor-pointer border-2 border-[#2a3142]"
              />
            </div>

            {/* Title Color */}
            <div>
              <label className="block text-sm font-semibold text-gray-200 mb-2">Title Color</label>
              <input
                type="color"
                value={settings.titleColor}
                onChange={(e) => onChange({ ...settings, titleColor: e.target.value })}
                className="w-full h-12 rounded-lg cursor-pointer border-2 border-[#2a3142]"
              />
            </div>

            {/* Description Color */}
            <div>
              <label className="block text-sm font-semibold text-gray-200 mb-2">Description Color</label>
              <input
                type="color"
                value={settings.descriptionColor}
                onChange={(e) => onChange({ ...settings, descriptionColor: e.target.value })}
                className="w-full h-12 rounded-lg cursor-pointer border-2 border-[#2a3142]"
              />
            </div>
          </div>

          {/* Background Opacity */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <label className="block text-sm font-semibold text-gray-200">Background Opacity</label>
              <span className="text-sm font-bold" style={{ color: getRarityColor() }}>
                {settings.backgroundOpacity}%
              </span>
            </div>
            <input
              type="range"
              min="0"
              max="100"
              step="5"
              value={settings.backgroundOpacity}
              onChange={(e) => onChange({ ...settings, backgroundOpacity: parseInt(e.target.value) })}
              className="w-full h-3 rounded-lg appearance-none cursor-pointer"
              style={{
                background: `linear-gradient(to right, ${getRarityColor()} 0%, ${getRarityColor()} ${settings.backgroundOpacity}%, rgb(15, 20, 32) ${settings.backgroundOpacity}%, rgb(15, 20, 32) 100%)`,
              }}
            />
          </div>

          {/* Glow Effect */}
          <div className="flex items-center justify-between bg-[#1a1f3a] p-4 rounded-lg border-2 border-[#2a3142]">
            <div className="flex items-center gap-4 flex-1">
              <div>
                <h4 className="font-semibold text-white">Glow Effect</h4>
                <p className="text-sm text-gray-400">Add a glowing border effect</p>
              </div>
              {settings.glowEffect && (
                <input
                  type="color"
                  value={settings.glowColor}
                  onChange={(e) => onChange({ ...settings, glowColor: e.target.value })}
                  className="h-10 w-20 rounded-lg cursor-pointer border-2 border-[#2a3142]"
                />
              )}
            </div>
            <button
              onClick={() => onChange({ ...settings, glowEffect: !settings.glowEffect })}
              className={`relative w-16 h-9 rounded-full transition-all shadow-inner ${
                settings.glowEffect ? 'bg-blue-600' : 'bg-gray-700'
              }`}
            >
              <div
                className={`absolute top-1 left-1 w-7 h-7 bg-white rounded-full shadow-lg transition-transform ${
                  settings.glowEffect ? 'transform translate-x-7' : ''
                }`}
              />
            </button>
          </div>

          {/* Position & Scaling */}
          <div className="grid grid-cols-2 gap-4">
            {/* Position */}
            <div>
              <label className="block text-sm font-semibold text-gray-200 mb-2">Position</label>
              <select
                value={settings.position}
                onChange={(e) => onChange({ ...settings, position: e.target.value as NotificationPosition })}
                className="w-full bg-[#1a1f3a] border-2 border-[#2a3142] rounded-lg px-4 py-3 text-white focus:outline-none focus:border-blue-500"
              >
                {POSITIONS.map((pos) => (
                  <option key={pos} value={pos}>
                    {pos.split('-').map(w => w.charAt(0).toUpperCase() + w.slice(1)).join(' ')}
                  </option>
                ))}
              </select>
            </div>

            {/* Scaling */}
            <div>
              <label className="block text-sm font-semibold text-gray-200 mb-2">Scaling</label>
              <select
                value={settings.scaling}
                onChange={(e) => onChange({ ...settings, scaling: parseInt(e.target.value) as ScalingOption })}
                className="w-full bg-[#1a1f3a] border-2 border-[#2a3142] rounded-lg px-4 py-3 text-white focus:outline-none focus:border-blue-500"
              >
                {SCALING_OPTIONS.map((scale) => (
                  <option key={scale} value={scale}>
                    {scale}%
                  </option>
                ))}
              </select>
            </div>
          </div>

          {/* Icon */}
          <div>
            <label className="block text-sm font-semibold text-gray-200 mb-2">Icon (Emoji or Image)</label>
            <div className="flex gap-3">
              <input
                type="text"
                value={settings.icon}
                onChange={(e) => onChange({ ...settings, icon: e.target.value })}
                placeholder="Enter emoji or file path"
                className="flex-1 bg-[#1a1f3a] border-2 border-[#2a3142] rounded-lg px-4 py-3 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
              />
              {/* Show remove button if it's a file path */}
              {(settings.icon.includes('\\') || settings.icon.includes('/') || /\.(png|jpg|jpeg|gif|bmp|svg|ico|webp)$/i.test(settings.icon)) && (
                <button
                  onClick={() => {
                    // Reset to default emoji for this rarity
                    const defaultEmojis: Record<RarityTier, string> = {
                      'Common': '⭐',
                      'Uncommon': '🌟',
                      'Rare': '💎',
                      'Ultra Rare': '👑',
                      'Legendary': '🏆',
                    };
                    onChange({ ...settings, icon: defaultEmojis[rarity] });
                  }}
                  className="bg-red-600 hover:bg-red-500 px-4 py-3 rounded-lg font-semibold transition-all"
                >
                  <X className="w-4 h-4" />
                </button>
              )}
              <button
                onClick={() => handleFileSelect('icon')}
                className="bg-blue-600 hover:bg-blue-500 px-4 py-3 rounded-lg font-semibold transition-all flex items-center gap-2"
              >
                <Upload className="w-4 h-4" />
                Browse
              </button>
            </div>
            <p className="text-xs text-gray-400 mt-2">Supported formats: PNG, JPG, GIF, BMP, SVG, ICO, WEBP</p>
          </div>

          {/* Sound */}
          <div>
            <label className="block text-sm font-semibold text-gray-200 mb-2 flex items-center gap-2">
              <Volume2 className="w-4 h-4" />
              Custom Sound
            </label>
            <div className="flex gap-3">
              <input
                type="text"
                value={settings.soundPath || ''}
                readOnly
                placeholder="No sound selected"
                className="flex-1 bg-[#1a1f3a] border-2 border-[#2a3142] rounded-lg px-4 py-3 text-white placeholder-gray-500"
              />
              {settings.soundPath && (
                <button
                  onClick={() => onChange({ ...settings, soundPath: null })}
                  className="bg-red-600 hover:bg-red-500 px-4 py-3 rounded-lg font-semibold transition-all"
                >
                  <X className="w-4 h-4" />
                </button>
              )}
              <button
                onClick={() => handleFileSelect('sound')}
                className="bg-blue-600 hover:bg-blue-500 px-4 py-3 rounded-lg font-semibold transition-all flex items-center gap-2"
              >
                <Upload className="w-4 h-4" />
                Browse
              </button>
            </div>
            <p className="text-xs text-gray-400 mt-2">Supported formats: MP3, WAV, OGG, FLAC</p>
          </div>

          {/* Font */}
          <div>
            <label className="block text-sm font-semibold text-gray-200 mb-2 flex items-center gap-2">
              <Type className="w-4 h-4" />
              Custom Font
            </label>
            <div className="flex gap-3">
              <input
                type="text"
                value={settings.fontPath || ''}
                readOnly
                placeholder="No font selected"
                className="flex-1 bg-[#1a1f3a] border-2 border-[#2a3142] rounded-lg px-4 py-3 text-white placeholder-gray-500"
              />
              {settings.fontPath && (
                <button
                  onClick={() => onChange({ ...settings, fontPath: null })}
                  className="bg-red-600 hover:bg-red-500 px-4 py-3 rounded-lg font-semibold transition-all"
                >
                  <X className="w-4 h-4" />
                </button>
              )}
              <button
                onClick={() => handleFileSelect('font')}
                className="bg-blue-600 hover:bg-blue-500 px-4 py-3 rounded-lg font-semibold transition-all flex items-center gap-2"
              >
                <Upload className="w-4 h-4" />
                Browse
              </button>
            </div>
            <p className="text-xs text-gray-400 mt-2">Supported formats: TTF, OTF, WOFF, WOFF2</p>
          </div>
        </div>
      )}
    </div>
  );
}
