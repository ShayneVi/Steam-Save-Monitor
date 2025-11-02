import React, { useState, useEffect, useRef } from 'react';
import { Settings, Save, FolderOpen, CheckCircle, AlertCircle, Info, GamepadIcon, Search, Trash2, X, Trophy, Download, RefreshCw, Plus, Ban } from 'lucide-react';
import { invoke } from '@tauri-apps/api/tauri';
import { listen, emit } from '@tauri-apps/api/event';
import { AchievementToastContainer } from './components/AchievementToast';
import { RarityCustomizer } from './components/RarityCustomizer';
import { RaritySettings, defaultRaritySettings, RarityTier } from './types/rarityTypes';

type Tab = 'settings' | 'games' | 'achievements' | 'exclusions' | 'customization';

interface Config {
  ludusaviPath: string;
  backupPath: string;
  autoStart: boolean;
  notificationsEnabled: boolean;
  gameExecutables: { [gameName: string]: string };
  steamApiKey?: string;
  steamUserId?: string;
  steamId64?: string;
}

interface Achievement {
  id?: number;
  app_id: number;
  game_name: string;
  achievement_id: string;
  display_name: string;
  description: string;
  icon_url?: string;
  icon_gray_url?: string;
  hidden: boolean;
  achieved: boolean;
  unlock_time?: number;
  source: string;
  last_updated: number;
  global_unlock_percentage?: number;
}

interface GameAchievementSummary {
  app_id: number;
  game_name: string;
  total_achievements: number;
  unlocked_achievements: number;
  source: string;
  last_updated: number;
}

interface SteamGameSearchResult {
  app_id: number;
  name: string;
  header_image?: string;
}

interface SourceOption {
  name: string;
  unlocked_count: number;
  total_count: number;
}

interface AchievementSettings {
  duration: number; // in seconds
}

interface Exclusion {
  id?: number;
  app_id: number;
  name: string;
  added_at: number;
}

function App() {
  const [activeTab, setActiveTab] = useState<Tab>('settings');
  const [config, setConfig] = useState<Config>({
    ludusaviPath: '',
    backupPath: '',
    autoStart: true,
    notificationsEnabled: true,
    gameExecutables: {}
  });
  
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
  const [testingLudusavi, setTestingLudusavi] = useState(false);
  
  const [ludusaviGames, setLudusaviGames] = useState<string[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [filteredGames, setFilteredGames] = useState<string[]>([]);
  const [loadingManifest, setLoadingManifest] = useState(false);
  const [isSearching, setIsSearching] = useState(false);
  const [expandedSections, setExpandedSections] = useState<Set<string>>(new Set());

  // Achievement state
  const [achievementGames, setAchievementGames] = useState<GameAchievementSummary[]>([]);
  const [selectedGame, setSelectedGame] = useState<GameAchievementSummary | null>(null);
  const [gameAchievements, setGameAchievements] = useState<Achievement[]>([]);
  const [loadingAchievements, setLoadingAchievements] = useState(false);
  const [syncingAchievements, setSyncingAchievements] = useState(false);
  const [showManualAddForm, setShowManualAddForm] = useState(false);

  // Steam game search state
  const [steamSearchQuery, setSteamSearchQuery] = useState('');
  const [steamSearchResults, setSteamSearchResults] = useState<SteamGameSearchResult[]>([]);
  const [searchingSteam, setSearchingSteam] = useState(false);
  const steamSearchTimerRef = useRef<NodeJS.Timeout | null>(null);

  // Icon cache state - stores base64 data URLs
  const [iconCache, setIconCache] = useState<{ [url: string]: string }>({});

  // Edit achievement modal state
  const [editingAchievement, setEditingAchievement] = useState<Achievement | null>(null);
  const [editAchieved, setEditAchieved] = useState(false);
  const [editUnlockTime, setEditUnlockTime] = useState<string>('');

  // Source selection modal state
  const [sourceSelectionGame, setSourceSelectionGame] = useState<SteamGameSearchResult | null>(null);
  const [availableSources, setAvailableSources] = useState<SourceOption[]>([]);
  const [checkingSources, setCheckingSources] = useState(false);

  // Debounce timer ref
  const debounceTimerRef = useRef<NodeJS.Timeout | null>(null);

  // Achievement customization settings
  const [achievementSettings, setAchievementSettings] = useState<AchievementSettings>({ duration: 6 });

  // Rarity settings
  const [raritySettings, setRaritySettings] = useState<RaritySettings>(() => {
    const saved = localStorage.getItem('raritySettings');
    if (saved) {
      try {
        return JSON.parse(saved);
      } catch {
        return defaultRaritySettings;
      }
    }
    return defaultRaritySettings;
  });

  // Exclusions state
  const [exclusions, setExclusions] = useState<Exclusion[]>([]);
  const [loadingExclusions, setLoadingExclusions] = useState(false);
  const [exclusionSearchQuery, setExclusionSearchQuery] = useState('');
  const [exclusionSearchResults, setExclusionSearchResults] = useState<SteamGameSearchResult[]>([]);
  const [searchingExclusions, setSearchingExclusions] = useState(false);
  const exclusionSearchTimerRef = useRef<NodeJS.Timeout | null>(null);

  const groupGamesByLetter = (games: string[]) => {
    const groups: { [key: string]: string[] } = {};
    
    if (!games || games.length === 0) return groups;
    
    games.forEach(game => {
      if (!game || game.length === 0) return;
      
      const firstChar = game.charAt(0).toUpperCase();
      const key = /[0-9]/.test(firstChar) ? '0-9' : firstChar;
      
      if (!groups[key]) {
        groups[key] = [];
      }
      groups[key].push(game);
    });
    
    // Sort each group
    Object.keys(groups).forEach(key => {
      groups[key].sort();
    });
    
    return groups;
  };

  const toggleSection = (section: string) => {
    const newExpanded = new Set(expandedSections);
    if (newExpanded.has(section)) {
      newExpanded.delete(section);
    } else {
      newExpanded.add(section);
    }
    setExpandedSections(newExpanded);
  };

  // Exclusions functions
  const loadExclusions = async () => {
    setLoadingExclusions(true);
    try {
      const result = await invoke<Exclusion[]>('get_all_exclusions');
      setExclusions(result);
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to load exclusions: ${error}`
      });
    } finally {
      setLoadingExclusions(false);
    }
  };

  const handleAddExclusion = async (appId: number, name: string) => {
    try {
      await invoke('add_exclusion', { appId, name });
      setMessage({
        type: 'success',
        text: `Added ${name} to exclusions`
      });
      loadExclusions();
      setExclusionSearchQuery('');
      setExclusionSearchResults([]);
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to add exclusion: ${error}`
      });
    }
  };

  const handleRemoveExclusion = async (appId: number, name: string) => {
    try {
      await invoke('remove_exclusion', { appId });
      setMessage({
        type: 'success',
        text: `Removed ${name} from exclusions`
      });
      loadExclusions();
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to remove exclusion: ${error}`
      });
    }
  };

  const handleExclusionSearch = async (query: string) => {
    setExclusionSearchQuery(query);

    if (exclusionSearchTimerRef.current) {
      clearTimeout(exclusionSearchTimerRef.current);
    }

    if (query.trim().length < 2) {
      setExclusionSearchResults([]);
      return;
    }

    exclusionSearchTimerRef.current = setTimeout(async () => {
      setSearchingExclusions(true);
      try {
        const results = await invoke<SteamGameSearchResult[]>('search_steam_games', { query });
        setExclusionSearchResults(results);
      } catch (error) {
        setMessage({
          type: 'error',
          text: `Failed to search Steam games: ${error}`
        });
      } finally {
        setSearchingExclusions(false);
      }
    }, 500);
  };

  useEffect(() => {
    loadConfig();
    loadAllAchievements(); // Load achievements on app start for the tab badge

    // Load achievement duration from backend
    invoke<number>('get_achievement_duration')
      .then(duration => {
        console.log('[App] Loaded duration from backend:', duration);
        setAchievementSettings({ duration });
      })
      .catch(error => {
        console.error('[App] Failed to load duration from backend:', error);
      });

    // Listen for game not found events
    const unsubscribeNotFound = listen('game-not-found', (event: any) => {
      setMessage({
        type: 'error',
        text: `Game "${event.payload.name}" not found in Ludusavi manifest. Please add it manually in the Games tab.`
      });
    });

    // Listen for game detected events
    const unsubscribeDetected = listen('game-detected', (event: any) => {
      setMessage({
        type: 'success',
        text: `Game Save Monitor detected: ${event.payload}`
      });
      setTimeout(() => setMessage(null), 5000);
    });

    return () => {
      unsubscribeNotFound.then(fn => fn());
      unsubscribeDetected.then(fn => fn());
    };
  }, []);

  // Save achievement duration to backend whenever it changes
  useEffect(() => {
    // Call backend to set duration
    invoke('set_achievement_duration', { duration: achievementSettings.duration })
      .then(() => {
        console.log('[App] Duration saved to backend:', achievementSettings.duration);
      })
      .catch(error => {
        console.error('[App] Failed to save duration to backend:', error);
      });
  }, [achievementSettings]);

  // Save rarity settings to localStorage whenever they change
  useEffect(() => {
    localStorage.setItem('raritySettings', JSON.stringify(raritySettings));
  }, [raritySettings]);

  // Sync rarity settings to overlay whenever they change
  useEffect(() => {
    // Send rarity settings to overlay via backend (reaches ALL windows)
    invoke('sync_settings_to_overlay', {
      achievementSettings: {}, // Not used anymore - overlay gets duration from backend
      raritySettings
    }).catch((error) => {
      console.error('Failed to sync rarity settings to overlay:', error);
    });
  }, [raritySettings]);

  // Debounced search effect
  useEffect(() => {
    // Clear previous timer
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current);
    }

    // If search is empty, show all games immediately
    if (!searchQuery.trim()) {
      setFilteredGames(ludusaviGames);
      setIsSearching(false);
      return;
    }

    // Set searching state
    setIsSearching(true);

    // Set new debounce timer (1000ms delay)
    debounceTimerRef.current = setTimeout(() => {
      const query = searchQuery.toLowerCase();
      const results = ludusaviGames.filter(game =>
        game.toLowerCase().includes(query)
      );
      setFilteredGames(results);
      setIsSearching(false);
    }, 200);

    // Cleanup on unmount
    return () => {
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
      }
    };
  }, [searchQuery, ludusaviGames]);

  // Debounced Steam search effect
  useEffect(() => {
    if (steamSearchTimerRef.current) {
      clearTimeout(steamSearchTimerRef.current);
    }

    if (!steamSearchQuery.trim()) {
      setSteamSearchResults([]);
      setSearchingSteam(false);
      return;
    }

    setSearchingSteam(true);

    steamSearchTimerRef.current = setTimeout(async () => {
      try {
        const results = await invoke<SteamGameSearchResult[]>('search_steam_games', {
          query: steamSearchQuery
        });
        setSteamSearchResults(results);
      } catch (error) {
        console.error('Failed to search Steam games:', error);
        setMessage({
          type: 'error',
          text: `Failed to search: ${error}`
        });
        setSteamSearchResults([]);
      } finally {
        setSearchingSteam(false);
      }
    }, 500);

    return () => {
      if (steamSearchTimerRef.current) {
        clearTimeout(steamSearchTimerRef.current);
      }
    };
  }, [steamSearchQuery]);

  // Listen for achievement unlock events and update UI
  useEffect(() => {
    const setupListener = async () => {
      const unlisten = await listen<Achievement>('achievement-unlocked', (event) => {
        const unlockedAch = event.payload;
        console.log('🏆 Achievement unlocked event received in App:', unlockedAch);

        // Update gameAchievements if this achievement is for the currently selected game
        setGameAchievements(prev => {
          if (prev.length === 0) return prev;
          const index = prev.findIndex(a => a.achievement_id === unlockedAch.achievement_id);
          if (index !== -1) {
            const updated = [...prev];
            updated[index] = { ...updated[index], achieved: true, unlock_time: unlockedAch.unlock_time };
            return updated;
          }
          return prev;
        });

        // Update achievementGames to increment unlocked count
        setAchievementGames(prev => {
          const gameIndex = prev.findIndex(g => g.app_id === unlockedAch.app_id);
          if (gameIndex !== -1) {
            const updated = [...prev];
            updated[gameIndex] = {
              ...updated[gameIndex],
              unlocked_achievements: updated[gameIndex].unlocked_achievements + 1
            };
            return updated;
          }
          return prev;
        });
      });

      return unlisten;
    };

    let unlistenPromise = setupListener();

    return () => {
      unlistenPromise.then(unlisten => unlisten());
    };
  }, []);

  const loadConfig = async () => {
    try {
      const loadedConfig = await invoke<Config>('get_config');
      setConfig(loadedConfig);
    } catch (error) {
      console.error('Failed to load config:', error);
    }
  };

  const loadLudusaviManifest = async () => {
    if (!config.ludusaviPath) {
      setMessage({
        type: 'error',
        text: 'Please configure Ludusavi path first'
      });
      return;
    }

    setLoadingManifest(true);
    try {
      const games = await invoke<string[]>('get_ludusavi_manifest');
      setLudusaviGames(games);
      setFilteredGames(games);
      // Auto-expand first section
      if (games.length > 0) {
        const firstChar = games[0].charAt(0).toUpperCase();
        const firstSection = /[0-9]/.test(firstChar) ? '0-9' : firstChar;
        setExpandedSections(new Set([firstSection]));
      }
      setMessage({
        type: 'success',
        text: `Loaded ${games.length} games from Ludusavi manifest (using cache - instant load!)`
      });
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to load Ludusavi manifest: ${error}`
      });
    } finally {
      setLoadingManifest(false);
    }
  };


  const handleSave = async () => {
    setSaving(true);
    setMessage(null);

    try {
      if (!config.ludusaviPath || !config.backupPath) {
        setMessage({
          type: 'error',
          text: 'Please fill in all required fields'
        });
        setSaving(false);
        return;
      }

      await invoke('save_config', { config });
      setMessage({
        type: 'success',
        text: 'Configuration saved successfully! Monitoring will restart.'
      });
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to save configuration: ${error}`
      });
    } finally {
      setSaving(false);
    }
  };

  const handleBrowseLudusavi = async () => {
    try {
      const path = await invoke<string | null>('browse_file');
      if (path) {
        setConfig({ ...config, ludusaviPath: path });
      }
    } catch (error) {
      console.error('Failed to browse file:', error);
    }
  };

  const handleBrowseBackup = async () => {
    try {
      const path = await invoke<string | null>('browse_folder');
      if (path) {
        setConfig({ ...config, backupPath: path });
      }
    } catch (error) {
      console.error('Failed to browse folder:', error);
    }
  };

  const handleTestLudusavi = async () => {
    if (!config.ludusaviPath) {
      setMessage({
        type: 'error',
        text: 'Please select Ludusavi executable first'
      });
      return;
    }

    setTestingLudusavi(true);
    setMessage(null);

    try {
      const result = await invoke<{ success: boolean; error?: string }>('test_ludusavi', { 
        path: config.ludusaviPath 
      });
      
      if (result.success) {
        setMessage({
          type: 'success',
          text: 'Ludusavi connection successful!'
        });
      } else {
        setMessage({
          type: 'error',
          text: `Ludusavi test failed: ${result.error || 'Unknown error'}`
        });
      }
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to test Ludusavi: ${error}`
      });
    } finally {
      setTestingLudusavi(false);
    }
  };

  const handleBrowseGameExe = async (gameName: string) => {
    try {
      const path = await invoke<string | null>('browse_file');
      if (path) {
        const updatedConfig = {
          ...config,
          gameExecutables: {
            ...config.gameExecutables,
            [gameName]: path
          }
        };
        
        setConfig(updatedConfig);
        
        // Auto-save
        setSaving(true);
        try {
          await invoke('save_config', { config: updatedConfig });
          setMessage({
            type: 'success',
            text: `Added executable for ${gameName} and saved configuration`
          });
          setTimeout(() => setMessage(null), 3000);
        } catch (error) {
          setMessage({
            type: 'error',
            text: `Failed to save configuration: ${error}`
          });
        } finally {
          setSaving(false);
        }
      }
    } catch (error) {
      console.error('Failed to browse file:', error);
    }
  };

  const handleRemoveGameExe = async (gameName: string) => {
    const newExes = { ...config.gameExecutables };
    delete newExes[gameName];
    
    const updatedConfig = {
      ...config,
      gameExecutables: newExes
    };
    
    setConfig(updatedConfig);
    
    // Auto-save
    setSaving(true);
    try {
      await invoke('save_config', { config: updatedConfig });
      setMessage({
        type: 'success',
        text: `Removed executable for ${gameName}`
      });
      setTimeout(() => setMessage(null), 3000);
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to save: ${error}`
      });
    } finally {
      setSaving(false);
    }
  };

  const loadAllAchievements = async () => {
    setLoadingAchievements(true);
    try {
      const games = await invoke<GameAchievementSummary[]>('get_all_achievements');
      setAchievementGames(games);
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to load achievements: ${error}`
      });
    } finally {
      setLoadingAchievements(false);
    }
  };

  const loadGameAchievements = async (game: GameAchievementSummary) => {
    setLoadingAchievements(true);
    setSelectedGame(game);
    try {
      const achievements = await invoke<Achievement[]>('get_game_achievements', { appId: game.app_id });
      // Debug: Log first achievement to see icon URLs
      if (achievements.length > 0) {
        console.log('First achievement data:', achievements[0]);
        console.log('Icon URL:', achievements[0].icon_url);
        console.log('Icon Gray URL:', achievements[0].icon_gray_url);
      }
      setGameAchievements(achievements);
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to load achievements for ${game.game_name}: ${error}`
      });
    } finally {
      setLoadingAchievements(false);
    }
  };

  const handleSyncAchievements = async () => {
    setSyncingAchievements(true);
    try {
      const result = await invoke<string>('sync_achievements');
      setMessage({
        type: 'success',
        text: result
      });
      // Reload achievements after sync
      await loadAllAchievements();
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to sync achievements: ${error}`
      });
    } finally {
      setSyncingAchievements(false);
    }
  };

  const handleAddGameToTracking = async (game: SteamGameSearchResult) => {
    try {
      setCheckingSources(true);
      setMessage({
        type: 'success' as any,
        text: `Checking sources for ${game.name}...`
      });

      // Call backend to check which sources have this game
      const sources = await invoke<SourceOption[]>('check_game_sources', {
        appId: game.app_id,
        gameName: game.name
      });

      setAvailableSources(sources);
      setSourceSelectionGame(game);
      setCheckingSources(false);
      setMessage(null);
    } catch (error) {
      setCheckingSources(false);
      setMessage({
        type: 'error',
        text: `Failed to check sources: ${error}`
      });
    }
  };

  const handleConfirmSourceSelection = async (source: string) => {
    if (!sourceSelectionGame) return;

    try {
      setMessage({
        type: 'success' as any,
        text: `Adding ${sourceSelectionGame.name} from ${source}...`
      });

      // Call backend to add from selected source
      const result = await invoke<string>('add_game_from_source', {
        appId: sourceSelectionGame.app_id,
        gameName: sourceSelectionGame.name,
        source: source
      });

      setMessage({
        type: 'success',
        text: result
      });

      // Check if backup exists for this game
      const backupPath = await invoke<string | null>('check_backup_exists', {
        gameName: sourceSelectionGame.name
      });

      if (backupPath) {
        // Ask user if they want to restore from backup
        const restore = window.confirm(
          `A backup file was found for ${sourceSelectionGame.name}.\n\n` +
          `Would you like to restore achievements from the backup?\n\n` +
          `The backup will be imported while still monitoring the selected source for new achievements.`
        );

        if (restore) {
          try {
            const restoredCount = await invoke<number>('restore_from_backup', {
              appId: sourceSelectionGame.app_id,
              gameName: sourceSelectionGame.name,
              backupPath: backupPath
            });

            setMessage({
              type: 'success',
              text: `Successfully restored ${restoredCount} achievements from backup. Now monitoring ${source} for new achievements.`
            });
          } catch (error) {
            setMessage({
              type: 'error',
              text: `Failed to restore backup: ${error}`
            });
          }
        }
      }

      // Reload achievement games list
      await loadAllAchievements();

      // Clear search and close modal
      setSteamSearchQuery('');
      setSteamSearchResults([]);
      setSourceSelectionGame(null);
      setAvailableSources([]);
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to add game: ${error}`
      });
    }
  };

  const handleExportGameAchievements = async (appId: number, gameName: string) => {
    try {
      const result = await invoke<string>('export_game_achievements', {
        appId: appId,
        gameName: gameName
      });
      setMessage({
        type: 'success',
        text: result
      });
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to export achievements: ${error}`
      });
    }
  };

  const handleRemoveGame = async (appId: number, gameName: string, event: React.MouseEvent) => {
    event.stopPropagation(); // Prevent opening game details when clicking remove

    try {
      const result = await invoke<string>('remove_game_from_tracking', { appId });
      setMessage({
        type: 'success',
        text: `Removed ${gameName}`
      });

      // Reload achievement games list
      await loadAllAchievements();

      // Close details if this was the selected game
      if (selectedGame?.app_id === appId) {
        setSelectedGame(null);
        setGameAchievements([]);
      }
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to remove game: ${error}`
      });
    }
  };

  // Helper function to get achievement icon (fetched through backend)
  const getAchievementIcon = async (url: string | undefined): Promise<string | undefined> => {
    if (!url) return undefined;

    // Check cache first
    if (iconCache[url]) {
      return iconCache[url];
    }

    // Fetch through backend
    try {
      const base64Data = await invoke<string>('fetch_achievement_icon', { url });
      // Cache it
      setIconCache(prev => ({ ...prev, [url]: base64Data }));
      return base64Data;
    } catch (error) {
      console.error('Failed to fetch icon:', error);
      return undefined;
    }
  };

  const handleAchievementClick = (achievement: Achievement) => {
    setEditingAchievement(achievement);
    setEditAchieved(achievement.achieved);

    // Convert Unix timestamp to datetime-local format
    if (achievement.unlock_time) {
      const date = new Date(achievement.unlock_time * 1000);
      const localDateTime = date.getFullYear() + '-' +
        String(date.getMonth() + 1).padStart(2, '0') + '-' +
        String(date.getDate()).padStart(2, '0') + 'T' +
        String(date.getHours()).padStart(2, '0') + ':' +
        String(date.getMinutes()).padStart(2, '0');
      setEditUnlockTime(localDateTime);
    } else {
      // Default to current time
      const now = new Date();
      const localDateTime = now.getFullYear() + '-' +
        String(now.getMonth() + 1).padStart(2, '0') + '-' +
        String(now.getDate()).padStart(2, '0') + 'T' +
        String(now.getHours()).padStart(2, '0') + ':' +
        String(now.getMinutes()).padStart(2, '0');
      setEditUnlockTime(localDateTime);
    }
  };

  const handleCloseEditModal = () => {
    setEditingAchievement(null);
    setEditAchieved(false);
    setEditUnlockTime('');
  };

  const handleSaveAchievement = async () => {
    if (!editingAchievement?.id) return;

    try {
      // Convert datetime-local to Unix timestamp
      let unlockTime: number | null = null;
      if (editAchieved && editUnlockTime) {
        const date = new Date(editUnlockTime);
        unlockTime = Math.floor(date.getTime() / 1000);
      }

      await invoke('update_achievement_status', {
        achievementId: editingAchievement.id,
        achieved: editAchieved,
        unlockTime: editAchieved ? unlockTime : null
      });

      setMessage({
        type: 'success',
        text: `Achievement "${editingAchievement.display_name}" updated successfully!`
      });

      // Reload achievements for this game
      if (selectedGame) {
        await loadGameAchievements(selectedGame);
      }

      // Reload all achievements to update the game card count
      await loadAllAchievements();

      // Close modal
      handleCloseEditModal();
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to update achievement: ${error}`
      });
    }
  };

  // Component for rendering achievement icons
  const AchievementIcon: React.FC<{ achievement: Achievement }> = ({ achievement }) => {
    const [hasError, setHasError] = useState(false);
    const iconUrl = achievement.achieved ? achievement.icon_url : achievement.icon_gray_url;

    // If no icon URL or previous error, show trophy
    if (!iconUrl || hasError) {
      return (
        <div className={`flex-shrink-0 p-3 rounded-lg ${
          achievement.achieved ? 'bg-emerald-600/20' : 'bg-gray-700/20'
        }`}>
          <Trophy className={`w-10 h-10 ${
            achievement.achieved ? 'text-emerald-400' : 'text-gray-600'
          }`} />
        </div>
      );
    }

    // Try to load the image directly with error handling
    return (
      <img
        src={iconUrl}
        alt={achievement.display_name}
        className="w-16 h-16 flex-shrink-0 rounded-lg border-2 border-[#2a3142] object-cover"
        onError={() => setHasError(true)}
      />
    );
  };

  const configuredGames = Object.keys(config.gameExecutables);

  return (
    <div className="min-h-screen bg-[#0a0e1a] text-white">
      {/* Header */}
      <div className="bg-gradient-to-r from-[#1a1f3a] to-[#2a2f4a] border-b border-[#3a4466] shadow-xl">
        <div className="max-w-7xl mx-auto px-8 py-6">
          <div className="flex items-center gap-4">
            <div className="p-3 bg-blue-600/20 rounded-xl border border-blue-500/30">
              <Save className="w-8 h-8 text-blue-400" />
            </div>
            <div>
              <h1 className="text-3xl font-bold text-white">Steam Backup Manager</h1>
              <p className="text-blue-300/80 text-sm mt-1">Automatic game save backups with Ludusavi</p>
            </div>
          </div>
        </div>
      </div>

      {/* Tabs */}
      <div className="bg-[#0f1420] border-b border-[#2a3142]">
        <div className="max-w-7xl mx-auto px-8">
          <div className="flex gap-1">
            <button
              onClick={() => setActiveTab('settings')}
              className={`px-6 py-4 font-semibold transition-all relative ${
                activeTab === 'settings'
                  ? 'text-blue-400 bg-[#1a1f3a]'
                  : 'text-gray-400 hover:text-gray-200 hover:bg-[#13172a]'
              }`}
            >
              <div className="flex items-center gap-2">
                <Settings className="w-5 h-5" />
                <span>Settings</span>
              </div>
              {activeTab === 'settings' && (
                <div className="absolute bottom-0 left-0 right-0 h-1 bg-blue-500 rounded-t" />
              )}
            </button>
            <button
              onClick={() => {
                setActiveTab('games');
                if (ludusaviGames.length === 0) {
                  loadLudusaviManifest();
                }
              }}
              className={`px-6 py-4 font-semibold transition-all relative ${
                activeTab === 'games'
                  ? 'text-blue-400 bg-[#1a1f3a]'
                  : 'text-gray-400 hover:text-gray-200 hover:bg-[#13172a]'
              }`}
            >
              <div className="flex items-center gap-2">
                <GamepadIcon className="w-5 h-5" />
                <span>Game Executables</span>
                <span className="ml-1 px-2 py-0.5 bg-blue-600/30 text-blue-300 text-xs rounded-full border border-blue-500/30">
                  {configuredGames.length}
                </span>
              </div>
              {activeTab === 'games' && (
                <div className="absolute bottom-0 left-0 right-0 h-1 bg-blue-500 rounded-t" />
              )}
            </button>
            <button
              onClick={() => {
                setActiveTab('exclusions');
                if (exclusions.length === 0) {
                  loadExclusions();
                }
              }}
              className={`px-6 py-4 font-semibold transition-all relative ${
                activeTab === 'exclusions'
                  ? 'text-blue-400 bg-[#1a1f3a]'
                  : 'text-gray-400 hover:text-gray-200 hover:bg-[#13172a]'
              }`}
            >
              <div className="flex items-center gap-2">
                <Ban className="w-5 h-5" />
                <span>Exclusions</span>
                <span className="ml-1 px-2 py-0.5 bg-red-600/30 text-red-300 text-xs rounded-full border border-red-500/30">
                  {exclusions.length}
                </span>
              </div>
              {activeTab === 'exclusions' && (
                <div className="absolute bottom-0 left-0 right-0 h-1 bg-blue-500 rounded-t" />
              )}
            </button>
            <button
              onClick={() => {
                setActiveTab('achievements');
                if (achievementGames.length === 0) {
                  loadAllAchievements();
                }
              }}
              className={`px-6 py-4 font-semibold transition-all relative ${
                activeTab === 'achievements'
                  ? 'text-blue-400 bg-[#1a1f3a]'
                  : 'text-gray-400 hover:text-gray-200 hover:bg-[#13172a]'
              }`}
            >
              <div className="flex items-center gap-2">
                <Trophy className="w-5 h-5" />
                <span>Achievements</span>
                <span className="ml-1 px-2 py-0.5 bg-amber-600/30 text-amber-300 text-xs rounded-full border border-amber-500/30">
                  {achievementGames.length}
                </span>
              </div>
              {activeTab === 'achievements' && (
                <div className="absolute bottom-0 left-0 right-0 h-1 bg-blue-500 rounded-t" />
              )}
            </button>
            <button
              onClick={() => setActiveTab('customization')}
              className={`px-6 py-4 font-semibold transition-all relative ${
                activeTab === 'customization'
                  ? 'text-blue-400 bg-[#1a1f3a]'
                  : 'text-gray-400 hover:text-gray-200 hover:bg-[#13172a]'
              }`}
            >
              <div className="flex items-center gap-2">
                <Settings className="w-5 h-5" />
                <span>Achievement Customization</span>
              </div>
              {activeTab === 'customization' && (
                <div className="absolute bottom-0 left-0 right-0 h-1 bg-blue-500 rounded-t" />
              )}
            </button>
          </div>
        </div>
      </div>

      <div className="max-w-7xl mx-auto px-8 py-8">
        {/* Message Banner */}
        {message && (
          <div className={`rounded-xl p-4 mb-6 flex items-center gap-3 border shadow-lg ${
            message.type === 'success' 
              ? 'bg-emerald-950/50 border-emerald-600/50 text-emerald-100' 
              : 'bg-red-950/50 border-red-600/50 text-red-100'
          }`}>
            {message.type === 'success' ? (
              <CheckCircle className="w-6 h-6 text-emerald-400 flex-shrink-0" />
            ) : (
              <AlertCircle className="w-6 h-6 text-red-400 flex-shrink-0" />
            )}
            <p className="flex-1 font-medium">{message.text}</p>
            <button
              onClick={() => setMessage(null)}
              className="p-1 hover:bg-white/10 rounded-lg transition-colors"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        )}

        {/* Settings Tab */}
        {activeTab === 'settings' && (
          <div className="space-y-6">
            {/* Info Box */}
            <div className="bg-blue-950/30 border border-blue-600/30 rounded-xl p-5 flex gap-4 shadow-lg">
              <Info className="w-6 h-6 text-blue-400 flex-shrink-0 mt-0.5" />
              <div className="text-sm text-blue-100/90 space-y-1">
                <p className="font-semibold text-blue-300 text-base mb-2">🎮 Steamworks Integration</p>
                <p>✓ This app uses Steamworks SDK for automatic Steam game detection</p>
                <p>✓ Achievement tracking requires a Steam Web API Key</p>
                <p>✓ Get your free API key at: <a href="https://steamcommunity.com/dev/apikey" target="_blank" rel="noopener noreferrer" className="text-blue-400 hover:text-blue-300 underline">steamcommunity.com/dev/apikey</a></p>
              </div>
            </div>

            {/* Settings Form */}
            <div className="bg-[#1a1f3a] rounded-xl p-8 border border-[#2a3142] shadow-xl space-y-8">
              <div className="flex items-center gap-3 pb-4 border-b border-[#2a3142]">
                <Settings className="w-7 h-7 text-blue-400" />
                <h2 className="text-2xl font-bold text-white">Configuration</h2>
              </div>

              {/* Ludusavi Path */}
              <div className="space-y-3">
                <label className="block text-sm font-semibold text-gray-200 flex items-center gap-2">
                  <FolderOpen className="w-4 h-4 text-blue-400" />
                  Ludusavi Executable Path
                  <span className="text-red-400">*</span>
                </label>
                <div className="flex gap-3">
                  <input
                    type="text"
                    value={config.ludusaviPath}
                    onChange={(e) => setConfig({ ...config, ludusaviPath: e.target.value })}
                    placeholder="C:\Program Files\Ludusavi\ludusavi.exe"
                    className="flex-1 bg-[#0f1420] border-2 border-[#2a3142] rounded-lg px-4 py-3.5 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-all font-mono text-sm"
                  />
                  <button
                    onClick={handleBrowseLudusavi}
                    className="bg-blue-600 hover:bg-blue-500 px-6 py-3.5 rounded-lg font-semibold transition-all shadow-lg hover:shadow-blue-500/20 border border-blue-500/30"
                  >
                    Browse
                  </button>
                  <button
                    onClick={handleTestLudusavi}
                    disabled={testingLudusavi}
                    className="bg-emerald-600 hover:bg-emerald-500 px-6 py-3.5 rounded-lg font-semibold transition-all shadow-lg hover:shadow-emerald-500/20 disabled:opacity-50 disabled:cursor-not-allowed border border-emerald-500/30"
                  >
                    {testingLudusavi ? 'Testing...' : 'Test'}
                  </button>
                </div>
              </div>

              {/* Backup Path */}
              <div className="space-y-3">
                <label className="block text-sm font-semibold text-gray-200 flex items-center gap-2">
                  <Save className="w-4 h-4 text-blue-400" />
                  Backup Directory
                  <span className="text-red-400">*</span>
                </label>
                <div className="flex gap-3">
                  <input
                    type="text"
                    value={config.backupPath}
                    onChange={(e) => setConfig({ ...config, backupPath: e.target.value })}
                    placeholder="C:\GameBackups"
                    className="flex-1 bg-[#0f1420] border-2 border-[#2a3142] rounded-lg px-4 py-3.5 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-all font-mono text-sm"
                  />
                  <button
                    onClick={handleBrowseBackup}
                    className="bg-blue-600 hover:bg-blue-500 px-6 py-3.5 rounded-lg font-semibold transition-all shadow-lg hover:shadow-blue-500/20 border border-blue-500/30"
                  >
                    Browse
                  </button>
                </div>
              </div>

              {/* Steam API Key */}
              <div className="space-y-3">
                <label className="block text-sm font-semibold text-gray-200 flex items-center gap-2">
                  <Trophy className="w-4 h-4 text-amber-400" />
                  Steam Web API Key
                  <span className="text-amber-400 text-xs">(Required for Achievements)</span>
                </label>
                <input
                  type="text"
                  value={config.steamApiKey || ''}
                  onChange={(e) => setConfig({ ...config, steamApiKey: e.target.value })}
                  placeholder="Get your API key from steamcommunity.com/dev/apikey"
                  className="w-full bg-[#0f1420] border-2 border-[#2a3142] rounded-lg px-4 py-3.5 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-all font-mono text-sm"
                />
                <p className="text-xs text-gray-400">
                  Your API key is stored locally and only used to fetch achievement data from Steam
                </p>
              </div>

              {/* Steam User ID */}
              <div className="space-y-3">
                <label className="block text-sm font-semibold text-gray-200 flex items-center gap-2">
                  <Trophy className="w-4 h-4 text-blue-400" />
                  Steam User ID
                  <span className="text-blue-400 text-xs">(Optional - for local achievement detection)</span>
                </label>
                <input
                  type="text"
                  value={config.steamUserId || ''}
                  onChange={(e) => setConfig({ ...config, steamUserId: e.target.value })}
                  placeholder="247367579"
                  className="w-full bg-[#0f1420] border-2 border-[#2a3142] rounded-lg px-4 py-3.5 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-all font-mono text-sm"
                />
                <p className="text-xs text-gray-400">
                  Your Steam3 ID number (find it in C:\Program Files (x86)\Steam\userdata\). If not set, the first user will be used.
                </p>
              </div>

              {/* Steam64 ID */}
              <div className="space-y-3">
                <label className="block text-sm font-semibold text-gray-200 flex items-center gap-2">
                  <Trophy className="w-4 h-4 text-purple-400" />
                  Steam64 ID
                  <span className="text-purple-400 text-xs">(Optional - for fetching your achievement unlock status from Steam API)</span>
                </label>
                <input
                  type="text"
                  value={config.steamId64 || ''}
                  onChange={(e) => setConfig({ ...config, steamId64: e.target.value })}
                  placeholder="76561198207633307"
                  className="w-full bg-[#0f1420] border-2 border-[#2a3142] rounded-lg px-4 py-3.5 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-all font-mono text-sm"
                />
                <p className="text-xs text-gray-400">
                  Your Steam64 ID (find it on steamid.io or steamidfinder.com). Used to fetch your personal achievement unlock times from Steam Web API.
                </p>
              </div>

              {/* Auto Start Toggle */}
              <div className="flex items-center justify-between bg-[#0f1420] p-5 rounded-lg border-2 border-[#2a3142]">
                <div>
                  <h3 className="font-semibold text-white text-base">Start with Windows</h3>
                  <p className="text-sm text-gray-400 mt-1">Launch app automatically when Windows starts</p>
                </div>
                <button
                  onClick={() => setConfig({ ...config, autoStart: !config.autoStart })}
                  className={`relative w-16 h-9 rounded-full transition-all shadow-inner ${
                    config.autoStart ? 'bg-blue-600' : 'bg-gray-700'
                  }`}
                >
                  <div
                    className={`absolute top-1 left-1 w-7 h-7 bg-white rounded-full shadow-lg transition-transform ${
                      config.autoStart ? 'transform translate-x-7' : ''
                    }`}
                  />
                </button>
              </div>

              {/* Notifications Toggle */}
              <div className="flex items-center justify-between bg-[#0f1420] p-5 rounded-lg border-2 border-[#2a3142]">
                <div>
                  <h3 className="font-semibold text-white text-base">Enable Notifications</h3>
                  <p className="text-sm text-gray-400 mt-1">Show Windows notifications for backup status</p>
                </div>
                <button
                  onClick={() => setConfig({ ...config, notificationsEnabled: !config.notificationsEnabled })}
                  className={`relative w-16 h-9 rounded-full transition-all shadow-inner ${
                    config.notificationsEnabled ? 'bg-blue-600' : 'bg-gray-700'
                  }`}
                >
                  <div
                    className={`absolute top-1 left-1 w-7 h-7 bg-white rounded-full shadow-lg transition-transform ${
                      config.notificationsEnabled ? 'transform translate-x-7' : ''
                    }`}
                  />
                </button>
              </div>

              {/* Save Button */}
              <div className="pt-3">
                <button
                  onClick={handleSave}
                  disabled={saving}
                  className="w-full bg-gradient-to-r from-emerald-600 to-emerald-500 hover:from-emerald-500 hover:to-emerald-400 text-white font-bold py-4 rounded-lg transition-all disabled:opacity-50 disabled:cursor-not-allowed shadow-lg hover:shadow-emerald-500/30 border border-emerald-400/30"
                >
                  {saving ? 'Saving Configuration...' : 'Save Configuration'}
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Games Tab */}
        {activeTab === 'games' && (
          <div className="space-y-6">
            {/* Header */}
            <div className="bg-[#1a1f3a] rounded-xl p-8 border border-[#2a3142] shadow-xl">
              <div className="flex items-center justify-between mb-6">
                <div className="flex items-center gap-3">
                  <div className="p-2 bg-blue-600/20 rounded-lg border border-blue-500/30">
                    <GamepadIcon className="w-6 h-6 text-blue-400" />
                  </div>
                  <div>
                    <h2 className="text-2xl font-bold text-white">Game Executables</h2>
                    <p className="text-gray-400 text-sm mt-1">
                      Map game executables to Ludusavi game names for process-based detection
                    </p>
                  </div>
                </div>
                <button
                  onClick={loadLudusaviManifest}
                  disabled={loadingManifest}
                  className="bg-blue-600 hover:bg-blue-500 px-5 py-3 rounded-lg font-semibold transition-all shadow-lg hover:shadow-blue-500/20 disabled:opacity-50 border border-blue-500/30"
                >
                  {loadingManifest ? 'Loading...' : 'Refresh Manifest'}
                </button>
              </div>

              {/* Search Bar */}
              <div className="relative">
                <Search className="absolute left-4 top-1/2 transform -translate-y-1/2 w-5 h-5 text-gray-400" />
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search for a game in Ludusavi manifest..."
                  className="w-full bg-[#0f1420] border-2 border-[#2a3142] rounded-lg pl-12 pr-4 py-4 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-all"
                />
                {isSearching && (
                  <div className="absolute right-4 top-1/2 transform -translate-y-1/2">
                    <div className="animate-spin rounded-full h-5 w-5 border-2 border-gray-700 border-t-blue-500"></div>
                  </div>
                )}
              </div>
            </div>

            {/* Configured Games */}
            {configuredGames.length > 0 && (
              <div className="bg-[#1a1f3a] rounded-xl p-8 border border-[#2a3142] shadow-xl">
                <h3 className="text-lg font-bold mb-5 flex items-center gap-2 text-white">
                  <CheckCircle className="w-6 h-6 text-emerald-400" />
                  Configured Games
                  <span className="ml-2 px-3 py-1 bg-emerald-600/30 text-emerald-300 text-sm rounded-full border border-emerald-500/30">
                    {configuredGames.length}
                  </span>
                </h3>
                <div className="space-y-3">
                  {configuredGames.map(gameName => (
                    <div key={gameName} className="bg-[#0f1420] border-2 border-[#2a3142] rounded-lg p-4 flex items-center justify-between hover:border-emerald-500/30 transition-all">
                      <div className="flex-1 min-w-0">
                        <p className="font-semibold text-white text-base">{gameName}</p>
                        <p className="text-sm text-gray-400 truncate font-mono mt-1">{config.gameExecutables[gameName]}</p>
                      </div>
                      <button
                        onClick={() => handleRemoveGameExe(gameName)}
                        className="ml-4 p-2.5 text-red-400 hover:text-red-300 hover:bg-red-950/50 rounded-lg transition-all border border-transparent hover:border-red-500/30"
                        title="Remove"
                      >
                        <Trash2 className="w-5 h-5" />
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Available Games */}
            <div className="bg-[#1a1f3a] rounded-xl border border-[#2a3142] shadow-xl overflow-hidden">
              <div className="p-6 border-b border-[#2a3142] bg-[#13172a]">
                <h3 className="text-lg font-bold text-white">
                  Available Games in Ludusavi Manifest
                </h3>
                {ludusaviGames.length > 0 && (
                  <p className="text-sm text-gray-400 mt-2">
                    Showing <span className="text-blue-400 font-semibold">{filteredGames.length}</span> of <span className="text-blue-400 font-semibold">{ludusaviGames.length}</span> games
                  </p>
                )}
              </div>
              
              <div className="max-h-[600px] overflow-y-auto" id="games-scroll-container">
                {loadingManifest ? (
                  <div className="p-12 text-center">
                    <div className="inline-block animate-spin rounded-full h-12 w-12 border-4 border-gray-700 border-t-blue-500 mb-4"></div>
                    <p className="text-gray-400 font-medium">Loading games from Ludusavi manifest...</p>
                  </div>
                ) : isSearching ? (
                  <div className="p-12 text-center">
                    <div className="inline-block animate-spin rounded-full h-8 w-8 border-3 border-gray-700 border-t-blue-500 mb-3"></div>
                    <p className="text-gray-400 font-medium">Searching...</p>
                  </div>
                ) : filteredGames.length === 0 ? (
                  <div className="p-12 text-center text-gray-400">
                    <GamepadIcon className="w-16 h-16 mx-auto mb-4 opacity-50" />
                    <p className="font-medium">
                      {searchQuery ? 'No games found matching your search' : 'Click "Refresh Manifest" to load games'}
                    </p>
                  </div>
                ) : (
                  <div>
                    {Object.entries(groupGamesByLetter(filteredGames)).sort().map(([section, games]) => (
                      <div key={section} className="border-b border-[#2a3142]">
                        <button
                          onClick={() => toggleSection(section)}
                          className="w-full p-4 hover:bg-[#13172a] transition-colors flex items-center justify-between font-semibold text-white"
                        >
                          <span className="flex items-center gap-3">
                            <span className="text-lg">{section}</span>
                            <span className="text-sm text-gray-400 bg-[#0f1420] px-2 py-1 rounded">
                              {games.length}
                            </span>
                          </span>
                          <span className={`transform transition-transform ${expandedSections.has(section) ? 'rotate-180' : ''}`}>
                            ▼
                          </span>
                        </button>
                        {expandedSections.has(section) && (
                          <div className="divide-y divide-[#2a3142] bg-[#0f1420]">
                            {games.map(gameName => {
                              const isConfigured = gameName in config.gameExecutables;
                              return (
                                <div key={gameName} className="p-4 hover:bg-[#13172a] transition-colors flex items-center justify-between">
                                  <span className={`font-medium ${isConfigured ? 'text-emerald-400' : 'text-gray-200'}`}>
                                    {gameName}
                                  </span>
                                  <button
                                    onClick={() => handleBrowseGameExe(gameName)}
                                    disabled={saving}
                                    className={`flex items-center gap-2 px-4 py-2.5 rounded-lg font-semibold transition-all shadow-md disabled:opacity-50 disabled:cursor-not-allowed ${
                                      isConfigured
                                        ? 'bg-emerald-950/50 text-emerald-400 border-2 border-emerald-600/40 hover:bg-emerald-900/50'
                                        : 'bg-blue-600 hover:bg-blue-500 text-white border-2 border-blue-500/30'
                                    }`}
                                  >
                                    <FolderOpen className="w-4 h-4" />
                                    {isConfigured ? 'Change EXE' : 'Select EXE'}
                                  </button>
                                </div>
                              );
                            })}
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Achievements Tab */}
        {activeTab === 'achievements' && (
          <div className="space-y-6">
            {/* Top Row: Add Game + Filter Games */}
            <div className="grid grid-cols-2 gap-6">
              {/* Left: Add New Game */}
              <div className="bg-[#1a1f3a] rounded-xl p-5 border border-[#2a3142] shadow-xl">
                <div className="flex items-center gap-2 mb-4">
                  <div className="p-1.5 bg-amber-600/20 rounded-lg border border-amber-500/30">
                    <Trophy className="w-5 h-5 text-amber-400" />
                  </div>
                  <h3 className="text-lg font-bold text-white">Add New Game</h3>
                </div>

                {/* Search Bar */}
                <div className="relative">
                  <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-gray-400" />
                  <input
                    type="text"
                    value={steamSearchQuery}
                    onChange={(e) => setSteamSearchQuery(e.target.value)}
                    placeholder="Search Steam games..."
                    className="w-full bg-[#0f1420] border-2 border-[#2a3142] rounded-lg pl-10 pr-3 py-2.5 text-white text-sm placeholder-gray-500 focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-all"
                  />
                  {searchingSteam && (
                    <div className="absolute right-3 top-1/2 transform -translate-y-1/2">
                      <div className="animate-spin rounded-full h-4 w-4 border-2 border-gray-700 border-t-blue-500"></div>
                    </div>
                  )}
                </div>

                {/* Search Results */}
                {steamSearchResults.length > 0 && (
                  <div className="mt-3 bg-[#0f1420] rounded-lg border border-[#2a3142] max-h-40 overflow-y-auto">
                    <div className="divide-y divide-[#2a3142]">
                      {steamSearchResults.map((game) => (
                        <button
                          key={game.app_id}
                          onClick={() => handleAddGameToTracking(game)}
                          className="w-full p-3 hover:bg-[#13172a] transition-colors flex items-center justify-between group text-left"
                        >
                          <div className="flex-1 min-w-0">
                            <p className="font-medium text-white text-sm group-hover:text-blue-400 transition-colors truncate">
                              {game.name}
                            </p>
                          </div>
                          <Plus className="w-4 h-4 text-blue-400 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0 ml-2" />
                        </button>
                      ))}
                    </div>
                  </div>
                )}
              </div>

              {/* Right: Filter Added Games */}
              <div className="bg-[#1a1f3a] rounded-xl p-5 border border-[#2a3142] shadow-xl">
                <div className="flex items-center justify-between mb-4">
                  <div className="flex items-center gap-2">
                    <div className="p-1.5 bg-blue-600/20 rounded-lg border border-blue-500/30">
                      <Search className="w-5 h-5 text-blue-400" />
                    </div>
                    <h3 className="text-lg font-bold text-white">Filter Games</h3>
                  </div>
                </div>

                {/* Filter Input */}
                <div className="relative">
                  <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-gray-400" />
                  <input
                    type="text"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder="Filter added games..."
                    className="w-full bg-[#0f1420] border-2 border-[#2a3142] rounded-lg pl-10 pr-3 py-2.5 text-white text-sm placeholder-gray-500 focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-all"
                  />
                </div>
              </div>
            </div>

            {/* Game Library Grid */}
            {loadingAchievements && achievementGames.length === 0 ? (
              <div className="bg-[#1a1f3a] rounded-xl p-12 border border-[#2a3142] shadow-xl text-center">
                <div className="inline-block animate-spin rounded-full h-12 w-12 border-4 border-gray-700 border-t-blue-500 mb-4"></div>
                <p className="text-gray-400 font-medium">Loading games...</p>
              </div>
            ) : achievementGames.length === 0 ? (
              <div className="bg-[#1a1f3a] rounded-xl p-12 border border-[#2a3142] shadow-xl text-center">
                <Trophy className="w-16 h-16 mx-auto mb-4 opacity-50 text-gray-600" />
                <p className="text-gray-400 font-medium mb-4">No games added yet</p>
                <p className="text-sm text-gray-500">Search for a Steam game above to start tracking achievements</p>
              </div>
            ) : (
              <div className="grid grid-cols-4 gap-4">
                {achievementGames
                  .filter(game =>
                    searchQuery.trim() === '' ||
                    game.game_name.toLowerCase().includes(searchQuery.toLowerCase())
                  )
                  .map(game => {
                    const percentage = Math.round((game.unlocked_achievements / game.total_achievements) * 100);
                    return (
                      <div
                        key={`${game.app_id}-${game.source}`}
                        className="bg-[#1a1f3a] rounded-xl border border-[#2a3142] shadow-xl hover:border-blue-500/50 transition-all cursor-pointer overflow-hidden group relative"
                        onClick={() => loadGameAchievements(game)}
                      >
                        {/* Remove Button */}
                        <button
                          onClick={(e) => handleRemoveGame(game.app_id, game.game_name, e)}
                          className="absolute top-2 right-2 z-10 p-1.5 bg-red-600/90 hover:bg-red-500 rounded-lg opacity-0 group-hover:opacity-100 transition-all shadow-lg border border-red-500/50"
                          title="Remove game"
                        >
                          <X className="w-4 h-4 text-white" />
                        </button>

                        {/* Game Header Image */}
                        <div className="h-32 relative overflow-hidden">
                          <img
                            src={`https://cdn.cloudflare.steamstatic.com/steam/apps/${game.app_id}/header.jpg`}
                            alt={game.game_name}
                            className="w-full h-full object-cover"
                            onError={(e) => {
                              // Fallback to gradient if image fails to load
                              e.currentTarget.style.display = 'none';
                              if (e.currentTarget.parentElement) {
                                e.currentTarget.parentElement.className = 'h-32 bg-gradient-to-br from-blue-900/30 to-purple-900/30 relative overflow-hidden';
                              }
                            }}
                          />
                          <div className="absolute inset-0 bg-black/40 group-hover:bg-black/20 transition-all" />
                          <div className="absolute bottom-2 left-3 right-3">
                            <div className="text-xs font-semibold text-white/90">
                              {game.unlocked_achievements} / {game.total_achievements}
                            </div>
                          </div>
                        </div>

                        {/* Game Info */}
                        <div className="p-3">
                          <h3 className="text-sm font-bold text-white line-clamp-2 mb-2 group-hover:text-blue-400 transition-colors">
                            {game.game_name}
                          </h3>

                          {/* Progress bar */}
                          <div className="bg-[#0f1420] rounded-full h-2 overflow-hidden">
                            <div
                              className="h-full bg-gradient-to-r from-blue-600 to-emerald-500 transition-all duration-500"
                              style={{ width: `${percentage}%` }}
                            />
                          </div>

                          <div className="flex items-center justify-between mt-2">
                            <span className="text-xs text-gray-400">{percentage}% Complete</span>
                            <span className="text-xs text-blue-400">{game.source}</span>
                          </div>
                        </div>
                      </div>
                    );
                  })}
              </div>
            )}

            {/* Achievement Details Modal/Panel - Keep existing */}
            {selectedGame && (
              <div className="bg-[#1a1f3a] rounded-xl border border-[#2a3142] shadow-xl overflow-hidden">
                <div className="p-6 bg-[#13172a] border-b border-[#2a3142]">
                  <div className="flex items-center justify-between">
                    <div>
                      <h3 className="text-2xl font-bold text-white">{selectedGame.game_name}</h3>
                      <p className="text-sm text-gray-400 mt-1">
                        {selectedGame.unlocked_achievements} of {selectedGame.total_achievements} achievements unlocked
                      </p>
                    </div>
                    <div className="flex items-center gap-3">
                      <button
                        onClick={() => handleExportGameAchievements(selectedGame.app_id, selectedGame.game_name)}
                        className="flex items-center gap-2 bg-emerald-600 hover:bg-emerald-500 px-4 py-2 rounded-lg font-semibold transition-all shadow-lg hover:shadow-emerald-500/20 border border-emerald-500/30"
                      >
                        <Download className="w-4 h-4" />
                        Export
                      </button>
                      <button
                        onClick={() => {
                          setSelectedGame(null);
                          setGameAchievements([]);
                        }}
                        className="p-2 hover:bg-white/10 rounded-lg transition-colors"
                      >
                        <X className="w-6 h-6" />
                      </button>
                    </div>
                  </div>
                </div>

                <div className="max-h-[600px] overflow-y-auto p-6 space-y-3">
                  {loadingAchievements ? (
                    <div className="text-center py-12">
                      <div className="inline-block animate-spin rounded-full h-8 w-8 border-3 border-gray-700 border-t-blue-500 mb-3"></div>
                      <p className="text-gray-400">Loading achievements...</p>
                    </div>
                  ) : gameAchievements.length === 0 ? (
                    <div className="text-center py-12 text-gray-400">
                      <p>No achievement details found</p>
                    </div>
                  ) : (
                    gameAchievements.map(achievement => (
                      <div
                        key={achievement.achievement_id}
                        onClick={() => handleAchievementClick(achievement)}
                        className={`p-4 rounded-lg border-2 transition-all cursor-pointer hover:scale-[1.02] ${
                          achievement.achieved
                            ? 'bg-emerald-950/30 border-emerald-600/40 hover:border-emerald-500/60'
                            : 'bg-[#0f1420] border-[#2a3142] hover:border-blue-500/60'
                        }`}
                      >
                        <div className="flex items-start gap-4">
                          {/* Achievement Icon */}
                          <AchievementIcon achievement={achievement} />
                          <div className="flex-1 min-w-0">
                            <h4 className={`font-bold ${
                              achievement.achieved ? 'text-emerald-300' : 'text-white'
                            }`}>
                              {achievement.display_name}
                            </h4>
                            {achievement.description && (
                              <p className="text-sm text-gray-400 mt-1">{achievement.description}</p>
                            )}
                            {achievement.global_unlock_percentage !== null && achievement.global_unlock_percentage !== undefined && (
                              <p className="text-xs text-blue-400 mt-2">
                                Global unlock rate: {achievement.global_unlock_percentage.toFixed(1)}%
                              </p>
                            )}
                            {achievement.unlock_time && (
                              <p className="text-xs text-gray-500 mt-2">
                                Unlocked: {new Date(achievement.unlock_time * 1000).toLocaleString()}
                              </p>
                            )}
                          </div>
                          {achievement.achieved && (
                            <CheckCircle className="flex-shrink-0 w-6 h-6 text-emerald-400" />
                          )}
                        </div>
                      </div>
                    ))
                  )}
                </div>
              </div>
            )}
          </div>
        )}

        {/* Exclusions Tab */}
        {activeTab === 'exclusions' && (
          <div className="space-y-6">
            {/* Top Row: Add Exclusion Search */}
            <div className="bg-[#1a1f3a] rounded-xl p-5 border border-[#2a3142] shadow-xl">
              <div className="flex items-center gap-2 mb-4">
                <div className="p-1.5 bg-red-600/20 rounded-lg border border-red-500/30">
                  <Ban className="w-5 h-5 text-red-400" />
                </div>
                <h3 className="text-lg font-bold text-white">Add App/Game to Exclusions</h3>
              </div>
              <p className="text-sm text-gray-400 mb-4">
                Excluded apps will not be detected or monitored for achievements. Perfect for utility apps like Wallpaper Engine, Borderless Gaming, etc.
              </p>

              {/* Search Bar */}
              <div className="relative">
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-gray-400" />
                <input
                  type="text"
                  value={exclusionSearchQuery}
                  onChange={(e) => handleExclusionSearch(e.target.value)}
                  placeholder="Search Steam apps and games..."
                  className="w-full bg-[#0f1420] border-2 border-[#2a3142] rounded-lg pl-10 pr-3 py-2.5 text-white text-sm placeholder-gray-500 focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-all"
                />
                {searchingExclusions && (
                  <div className="absolute right-3 top-1/2 transform -translate-y-1/2">
                    <div className="animate-spin rounded-full h-4 w-4 border-2 border-gray-700 border-t-blue-500"></div>
                  </div>
                )}
              </div>

              {/* Search Results */}
              {exclusionSearchResults.length > 0 && (
                <div className="mt-3 bg-[#0f1420] rounded-lg border border-[#2a3142] max-h-60 overflow-y-auto">
                  <div className="divide-y divide-[#2a3142]">
                    {exclusionSearchResults.map((app) => {
                      const isAlreadyExcluded = exclusions.some(e => e.app_id === app.app_id);
                      return (
                        <button
                          key={app.app_id}
                          onClick={() => !isAlreadyExcluded && handleAddExclusion(app.app_id, app.name)}
                          disabled={isAlreadyExcluded}
                          className={`w-full p-3 transition-colors flex items-center justify-between group text-left ${
                            isAlreadyExcluded
                              ? 'opacity-50 cursor-not-allowed'
                              : 'hover:bg-[#13172a] cursor-pointer'
                          }`}
                        >
                          <div className="flex-1 min-w-0">
                            <p className={`font-medium text-sm truncate ${
                              isAlreadyExcluded
                                ? 'text-gray-500'
                                : 'text-white group-hover:text-red-400 transition-colors'
                            }`}>
                              {app.name}
                            </p>
                            <p className="text-xs text-gray-500 mt-0.5">AppID: {app.app_id}</p>
                          </div>
                          {isAlreadyExcluded ? (
                            <CheckCircle className="w-4 h-4 text-gray-500 flex-shrink-0 ml-2" />
                          ) : (
                            <Ban className="w-4 h-4 text-red-400 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0 ml-2" />
                          )}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>

            {/* Excluded Apps List */}
            {loadingExclusions ? (
              <div className="bg-[#1a1f3a] rounded-xl p-12 border border-[#2a3142] shadow-xl text-center">
                <div className="inline-block animate-spin rounded-full h-12 w-12 border-4 border-gray-700 border-t-blue-500 mb-4"></div>
                <p className="text-gray-400 font-medium">Loading exclusions...</p>
              </div>
            ) : exclusions.length === 0 ? (
              <div className="bg-[#1a1f3a] rounded-xl p-12 border border-[#2a3142] shadow-xl text-center">
                <Ban className="w-16 h-16 mx-auto mb-4 opacity-50 text-gray-600" />
                <p className="text-gray-400 font-medium mb-4">No exclusions added yet</p>
                <p className="text-sm text-gray-500">Search for a Steam app above to exclude it from monitoring</p>
              </div>
            ) : (
              <div className="bg-[#1a1f3a] rounded-xl p-5 border border-[#2a3142] shadow-xl">
                <div className="flex items-center gap-2 mb-4">
                  <div className="p-1.5 bg-red-600/20 rounded-lg border border-red-500/30">
                    <Ban className="w-5 h-5 text-red-400" />
                  </div>
                  <h3 className="text-lg font-bold text-white">Excluded Apps ({exclusions.length})</h3>
                </div>
                <div className="space-y-2">
                  {exclusions.map((exclusion) => (
                    <div
                      key={exclusion.app_id}
                      className="bg-[#0f1420] rounded-lg p-4 border border-[#2a3142] hover:border-red-500/30 transition-all group"
                    >
                      <div className="flex items-center justify-between">
                        <div className="flex-1 min-w-0">
                          <p className="font-medium text-white text-sm truncate">
                            {exclusion.name}
                          </p>
                          <p className="text-xs text-gray-500 mt-1">AppID: {exclusion.app_id}</p>
                        </div>
                        <button
                          onClick={() => handleRemoveExclusion(exclusion.app_id, exclusion.name)}
                          className="ml-4 p-2 bg-red-600/20 hover:bg-red-600/30 rounded-lg border border-red-500/30 hover:border-red-500/50 transition-all group-hover:scale-105"
                          title="Remove from exclusions"
                        >
                          <Trash2 className="w-4 h-4 text-red-400" />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        {/* Customization Tab */}
        {activeTab === 'customization' && (
          <div className="space-y-6">
            {/* Achievement Customization Section */}
            <div className="bg-[#1a1f3a] rounded-xl p-8 border border-[#2a3142] shadow-xl space-y-8">
              <div className="flex items-center gap-3 pb-4 border-b border-[#2a3142]">
                <div className="p-2 bg-amber-600/20 rounded-lg border border-amber-500/30">
                  <Trophy className="w-7 h-7 text-amber-400" />
                </div>
                <div>
                  <h2 className="text-2xl font-bold text-white">Achievement Customization</h2>
                  <p className="text-gray-400 text-sm mt-1">Customize how achievement notifications appear</p>
                </div>
              </div>

              {/* Duration Slider */}
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <div>
                    <label className="block text-sm font-semibold text-gray-200">
                      Notification Duration
                    </label>
                    <p className="text-xs text-gray-400 mt-1">
                      How long achievement notifications stay on screen
                    </p>
                  </div>
                  <div className="text-right">
                    <span className="text-2xl font-bold text-blue-400">{achievementSettings.duration}</span>
                    <span className="text-sm text-gray-400 ml-1">seconds</span>
                  </div>
                </div>

                <div className="space-y-2">
                  <input
                    type="range"
                    min="2"
                    max="10"
                    step="1"
                    value={achievementSettings.duration}
                    onChange={(e) => setAchievementSettings({ ...achievementSettings, duration: parseInt(e.target.value) })}
                    className="w-full h-3 bg-[#0f1420] rounded-lg appearance-none cursor-pointer slider-thumb"
                    style={{
                      background: `linear-gradient(to right, rgb(59, 130, 246) 0%, rgb(59, 130, 246) ${((achievementSettings.duration - 2) / 8) * 100}%, rgb(15, 20, 32) ${((achievementSettings.duration - 2) / 8) * 100}%, rgb(15, 20, 32) 100%)`
                    }}
                  />
                  <div className="flex justify-between text-xs text-gray-500">
                    <span>2s</span>
                    <span>3s</span>
                    <span>4s</span>
                    <span>5s</span>
                    <span>6s</span>
                    <span>7s</span>
                    <span>8s</span>
                    <span>9s</span>
                    <span>10s</span>
                  </div>
                </div>
              </div>

              {/* Test Notification Button */}
              <div className="pt-6 border-t border-[#2a3142]">
                <button
                  onClick={async () => {
                    try {
                      await invoke('test_overlay');
                      console.log('Test overlay triggered');
                    } catch (error) {
                      console.error('Failed to test overlay:', error);
                    }
                  }}
                  className="w-full bg-gradient-to-r from-purple-600 to-purple-500 hover:from-purple-500 hover:to-purple-400 text-white font-bold py-3 rounded-lg transition-all shadow-lg hover:shadow-purple-500/30 border border-purple-400/30"
                >
                  🎮 Test Achievement Notification
                </button>
              </div>

              {/* Rarity Toggle */}
              <div className="pt-6 border-t border-[#2a3142]">
                <div className="flex items-center justify-between bg-[#0f1420] p-5 rounded-lg border-2 border-[#2a3142]">
                  <div>
                    <h3 className="font-semibold text-white text-base">Enable Rarity-Based Notifications</h3>
                    <p className="text-sm text-gray-400 mt-1">
                      Use different styles based on achievement rarity (determined by global unlock percentage)
                    </p>
                  </div>
                  <button
                    onClick={() => setRaritySettings({ ...raritySettings, enabled: !raritySettings.enabled })}
                    className={`relative w-16 h-9 rounded-full transition-all shadow-inner ${
                      raritySettings.enabled ? 'bg-blue-600' : 'bg-gray-700'
                    }`}
                  >
                    <div
                      className={`absolute top-1 left-1 w-7 h-7 bg-white rounded-full shadow-lg transition-transform ${
                        raritySettings.enabled ? 'transform translate-x-7' : ''
                      }`}
                    />
                  </button>
                </div>
              </div>

              {/* Rarity Customizers */}
              {raritySettings.enabled && (
                <div className="pt-6 border-t border-[#2a3142] space-y-4">
                  <div className="bg-blue-950/30 border border-blue-600/30 rounded-xl p-5 flex gap-4">
                    <Info className="w-6 h-6 text-blue-400 flex-shrink-0 mt-0.5" />
                    <div className="text-sm text-blue-100/90">
                      <p className="font-semibold text-blue-300 text-base mb-2">Rarity System</p>
                      <p>Achievements are categorized by their global unlock percentage:</p>
                      <ul className="list-disc list-inside mt-2 space-y-1">
                        <li><span className="font-semibold text-gray-300">Common:</span> 30%+ of players have unlocked</li>
                        <li><span className="font-semibold text-green-400">Uncommon:</span> 20-29% unlock rate</li>
                        <li><span className="font-semibold text-blue-400">Rare:</span> 13-19% unlock rate</li>
                        <li><span className="font-semibold text-purple-400">Ultra Rare:</span> 5-12% unlock rate</li>
                        <li><span className="font-semibold text-amber-400">Legendary:</span> 0-4% unlock rate</li>
                      </ul>
                    </div>
                  </div>

                  {/* Rarity Customizers */}
                  {(['Common', 'Uncommon', 'Rare', 'Ultra Rare', 'Legendary'] as RarityTier[]).map((rarity) => (
                    <RarityCustomizer
                      key={rarity}
                      rarity={rarity}
                      settings={raritySettings[rarity]}
                      onChange={(newSettings) => {
                        setRaritySettings({
                          ...raritySettings,
                          [rarity]: newSettings,
                        });
                      }}
                      onTest={async () => {
                        try {
                          await invoke('test_rarity_notification', { rarity });
                          console.log(`Test ${rarity} notification triggered`);
                        } catch (error) {
                          console.error(`Failed to test ${rarity} notification:`, error);
                        }
                      }}
                    />
                  ))}
                </div>
              )}

              {/* Preview Info */}
              <div className="bg-blue-950/30 border border-blue-600/30 rounded-xl p-5 flex gap-4">
                <Info className="w-6 h-6 text-blue-400 flex-shrink-0 mt-0.5" />
                <div className="text-sm text-blue-100/90 space-y-1">
                  <p className="font-semibold text-blue-300 text-base mb-2">Preview</p>
                  <p>Click the "Test Achievement Notification" button or individual rarity test buttons to preview your customization settings.</p>
                  <p className="text-xs text-blue-200/70 mt-2">Changes are saved automatically and will apply to all future achievement notifications.</p>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Achievement Edit Modal */}
      {editingAchievement && (
        <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-8" onClick={handleCloseEditModal}>
          <div className="bg-[#1a1f3a] rounded-xl border-2 border-[#2a3142] shadow-2xl w-full max-w-2xl" onClick={(e) => e.stopPropagation()}>
            {/* Modal Header */}
            <div className="p-6 border-b border-[#2a3142] bg-[#13172a]">
              <div className="flex items-start justify-between">
                <div className="flex items-start gap-4 flex-1">
                  <AchievementIcon achievement={editingAchievement} />
                  <div className="flex-1 min-w-0">
                    <h3 className="text-xl font-bold text-white">{editingAchievement.display_name}</h3>
                    {editingAchievement.description && (
                      <p className="text-sm text-gray-400 mt-1">{editingAchievement.description}</p>
                    )}
                  </div>
                </div>
                <button
                  onClick={handleCloseEditModal}
                  className="p-2 hover:bg-white/10 rounded-lg transition-colors ml-4"
                >
                  <X className="w-6 h-6" />
                </button>
              </div>
            </div>

            {/* Modal Body */}
            <div className="p-6 space-y-6">
              {/* Achievement Status Toggle */}
              <div className="flex items-center justify-between bg-[#0f1420] p-5 rounded-lg border-2 border-[#2a3142]">
                <div>
                  <h4 className="font-semibold text-white text-base">Achievement Status</h4>
                  <p className="text-sm text-gray-400 mt-1">
                    {editAchieved ? 'Marked as unlocked' : 'Marked as locked'}
                  </p>
                </div>
                <button
                  onClick={() => setEditAchieved(!editAchieved)}
                  className={`relative w-16 h-9 rounded-full transition-all shadow-inner ${
                    editAchieved ? 'bg-emerald-600' : 'bg-gray-700'
                  }`}
                >
                  <div
                    className={`absolute top-1 left-1 w-7 h-7 bg-white rounded-full shadow-lg transition-transform ${
                      editAchieved ? 'transform translate-x-7' : ''
                    }`}
                  />
                </button>
              </div>

              {/* Unlock Time Input */}
              {editAchieved && (
                <div className="space-y-3">
                  <label className="block text-sm font-semibold text-gray-200 flex items-center gap-2">
                    <Trophy className="w-4 h-4 text-amber-400" />
                    Unlock Date & Time
                  </label>
                  <input
                    type="datetime-local"
                    value={editUnlockTime}
                    onChange={(e) => setEditUnlockTime(e.target.value)}
                    className="w-full bg-[#0f1420] border-2 border-[#2a3142] rounded-lg px-4 py-3 text-white focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-all font-mono text-sm"
                  />

                  {/* Timestamp Converter */}
                  <div className="pt-2">
                    <label className="block text-xs font-semibold text-gray-400 mb-2">
                      Or paste Unix timestamp (seconds):
                    </label>
                    <input
                      type="text"
                      placeholder="e.g., 1234567890"
                      onPaste={(e) => {
                        const pastedText = e.clipboardData.getData('text');
                        const timestamp = parseInt(pastedText.trim());
                        if (!isNaN(timestamp) && timestamp > 0) {
                          const date = new Date(timestamp * 1000);
                          const localDateTime = date.getFullYear() + '-' +
                            String(date.getMonth() + 1).padStart(2, '0') + '-' +
                            String(date.getDate()).padStart(2, '0') + 'T' +
                            String(date.getHours()).padStart(2, '0') + ':' +
                            String(date.getMinutes()).padStart(2, '0');
                          setEditUnlockTime(localDateTime);
                          e.currentTarget.value = '';
                        }
                      }}
                      className="w-full bg-[#0f1420] border-2 border-[#2a3142] rounded-lg px-4 py-2 text-white text-sm placeholder-gray-500 focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-all font-mono"
                    />
                    <p className="text-xs text-gray-500 mt-1">
                      Paste a Unix timestamp and it will automatically convert to a date/time
                    </p>
                  </div>
                </div>
              )}

              {/* Source Info */}
              <div className="bg-blue-950/30 border border-blue-600/30 rounded-lg p-4 flex gap-3">
                <Info className="w-5 h-5 text-blue-400 flex-shrink-0 mt-0.5" />
                <div className="text-sm text-blue-100/90">
                  <p className="font-semibold text-blue-300 mb-1">Achievement Source: {editingAchievement.source}</p>
                  <p>Manual changes will override data from this source.</p>
                </div>
              </div>
            </div>

            {/* Modal Footer */}
            <div className="p-6 border-t border-[#2a3142] bg-[#13172a] flex gap-3">
              <button
                onClick={handleCloseEditModal}
                className="flex-1 bg-gray-700 hover:bg-gray-600 text-white font-semibold py-3 rounded-lg transition-all"
              >
                Cancel
              </button>
              <button
                onClick={handleSaveAchievement}
                className="flex-1 bg-gradient-to-r from-emerald-600 to-emerald-500 hover:from-emerald-500 hover:to-emerald-400 text-white font-semibold py-3 rounded-lg transition-all shadow-lg hover:shadow-emerald-500/30"
              >
                Save Changes
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Source Selection Modal */}
      {sourceSelectionGame && availableSources.length > 0 && (
        <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-8" onClick={() => {
          setSourceSelectionGame(null);
          setAvailableSources([]);
        }}>
          <div className="bg-[#1a1f3a] rounded-xl border-2 border-[#2a3142] shadow-2xl w-full max-w-2xl" onClick={(e) => e.stopPropagation()}>
            {/* Modal Header */}
            <div className="p-6 border-b border-[#2a3142] bg-[#13172a]">
              <div className="flex items-start justify-between">
                <div>
                  <h3 className="text-xl font-bold text-white">Select Achievement Source</h3>
                  <p className="text-sm text-gray-400 mt-1">Choose which source to use for {sourceSelectionGame.name}</p>
                </div>
                <button
                  onClick={() => {
                    setSourceSelectionGame(null);
                    setAvailableSources([]);
                  }}
                  className="p-2 hover:bg-white/10 rounded-lg transition-colors"
                >
                  <X className="w-6 h-6" />
                </button>
              </div>
            </div>

            {/* Modal Body */}
            <div className="p-6 space-y-4">
              {availableSources.map((source) => (
                <button
                  key={source.name}
                  onClick={() => handleConfirmSourceSelection(source.name)}
                  className="w-full bg-[#0f1420] hover:bg-[#13172a] border-2 border-[#2a3142] hover:border-blue-500 rounded-lg p-5 transition-all text-left group"
                >
                  <div className="flex items-center justify-between">
                    <div className="flex-1">
                      <h4 className="font-semibold text-white text-lg group-hover:text-blue-400 transition-colors">
                        {source.name}
                      </h4>
                      <div className="flex items-center gap-4 mt-2">
                        <div className="flex items-center gap-2">
                          <Trophy className="w-4 h-4 text-amber-400" />
                          <span className="text-sm text-gray-300">
                            <span className="font-semibold text-emerald-400">{source.unlocked_count}</span>
                            <span className="text-gray-500"> / </span>
                            <span className="font-semibold text-white">{source.total_count}</span>
                            <span className="text-gray-500 ml-1">achievements</span>
                          </span>
                        </div>
                        {source.unlocked_count > 0 && (
                          <div className="flex items-center gap-1 bg-emerald-500/20 border border-emerald-500/30 rounded px-2 py-0.5">
                            <CheckCircle className="w-3 h-3 text-emerald-400" />
                            <span className="text-xs text-emerald-300 font-medium">
                              {Math.round((source.unlocked_count / source.total_count) * 100)}% Complete
                            </span>
                          </div>
                        )}
                      </div>
                    </div>
                    <div className="text-blue-400 group-hover:translate-x-1 transition-transform">
                      →
                    </div>
                  </div>
                </button>
              ))}
            </div>

            {/* Modal Footer */}
            <div className="p-6 border-t border-[#2a3142] bg-[#0f1420] rounded-b-xl">
              <div className="flex gap-3 text-sm text-gray-400">
                <Info className="w-4 h-4 flex-shrink-0 mt-0.5" />
                <p>
                  Select the source with the most complete achievement data. You can manually add missing achievements later.
                </p>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Achievement Toast Notifications */}
      <AchievementToastContainer />
    </div>
  );
}

export default App;