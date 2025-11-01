// Rarity system types and utilities for achievement notifications

export type RarityTier = 'Common' | 'Uncommon' | 'Rare' | 'Ultra Rare' | 'Legendary';

export type NotificationPosition = 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right' | 'center';

export type ScalingOption = 40 | 60 | 80 | 100 | 120 | 140 | 160;

export interface RarityCustomization {
  borderColor: string;
  backgroundColor: string;
  backgroundOpacity: number; // 0-100
  glowEffect: boolean;
  glowColor: string;
  titleColor: string;
  descriptionColor: string;
  position: NotificationPosition;
  scaling: ScalingOption;
  icon: string; // emoji or file path
  soundPath: string | null; // file path to custom sound
  fontPath: string | null; // file path to custom font
}

export interface RaritySettings {
  enabled: boolean; // Master toggle for rarity system
  Common: RarityCustomization;
  Uncommon: RarityCustomization;
  Rare: RarityCustomization;
  'Ultra Rare': RarityCustomization;
  Legendary: RarityCustomization;
}

// Default rarity customizations with appropriate colors
export const defaultRaritySettings: RaritySettings = {
  enabled: false,
  Common: {
    borderColor: '#9CA3AF', // Gray
    backgroundColor: '#374151',
    backgroundOpacity: 90,
    glowEffect: false,
    glowColor: '#9CA3AF',
    titleColor: '#F3F4F6',
    descriptionColor: '#D1D5DB',
    position: 'top-right',
    scaling: 100,
    icon: '⭐',
    soundPath: null,
    fontPath: null,
  },
  Uncommon: {
    borderColor: '#10B981', // Green
    backgroundColor: '#065F46',
    backgroundOpacity: 90,
    glowEffect: true,
    glowColor: '#10B981',
    titleColor: '#D1FAE5',
    descriptionColor: '#A7F3D0',
    position: 'top-right',
    scaling: 100,
    icon: '🌟',
    soundPath: null,
    fontPath: null,
  },
  Rare: {
    borderColor: '#3B82F6', // Blue
    backgroundColor: '#1E3A8A',
    backgroundOpacity: 90,
    glowEffect: true,
    glowColor: '#3B82F6',
    titleColor: '#DBEAFE',
    descriptionColor: '#BFDBFE',
    position: 'top-right',
    scaling: 110,
    icon: '💎',
    soundPath: null,
    fontPath: null,
  },
  'Ultra Rare': {
    borderColor: '#A855F7', // Purple
    backgroundColor: '#581C87',
    backgroundOpacity: 95,
    glowEffect: true,
    glowColor: '#A855F7',
    titleColor: '#F3E8FF',
    descriptionColor: '#E9D5FF',
    position: 'top-right',
    scaling: 120,
    icon: '👑',
    soundPath: null,
    fontPath: null,
  },
  Legendary: {
    borderColor: '#F59E0B', // Gold
    backgroundColor: '#78350F',
    backgroundOpacity: 95,
    glowEffect: true,
    glowColor: '#F59E0B',
    titleColor: '#FEF3C7',
    descriptionColor: '#FDE68A',
    position: 'center',
    scaling: 140,
    icon: '🏆',
    soundPath: null,
    fontPath: null,
  },
};

/**
 * Calculate rarity based on global unlock percentage
 * @param percentage Global unlock percentage (0-100)
 * @returns The rarity tier
 */
export function calculateRarity(percentage: number | null | undefined): RarityTier {
  if (percentage === null || percentage === undefined) {
    return 'Common'; // Default for achievements without percentage data
  }

  if (percentage >= 90) return 'Common';
  if (percentage >= 60) return 'Uncommon';
  if (percentage >= 35) return 'Rare';
  if (percentage >= 15) return 'Ultra Rare';
  return 'Legendary';
}

/**
 * Get rarity color for display
 */
export function getRarityColor(rarity: RarityTier): string {
  switch (rarity) {
    case 'Common': return '#9CA3AF';
    case 'Uncommon': return '#10B981';
    case 'Rare': return '#3B82F6';
    case 'Ultra Rare': return '#A855F7';
    case 'Legendary': return '#F59E0B';
  }
}

/**
 * Format percentage for display
 */
export function formatPercentage(percentage: number | null | undefined): string {
  if (percentage === null || percentage === undefined) {
    return 'N/A';
  }
  return `${percentage.toFixed(1)}%`;
}
