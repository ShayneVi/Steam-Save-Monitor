import React, { useState, useEffect, useRef } from 'react';
import { Settings, Save, Key, FolderOpen, CheckCircle, AlertCircle, Info, GamepadIcon, Search, Trash2, X } from 'lucide-react';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';

type Tab = 'settings' | 'games';

interface Config {
  steamApiKey: string;
  steamUserId: string;
  ludusaviPath: string;
  backupPath: string;
  autoStart: boolean;
  notificationsEnabled: boolean;
  gameExecutables: { [gameName: string]: string };
}

function App() {
  const [activeTab, setActiveTab] = useState<Tab>('settings');
  const [config, setConfig] = useState<Config>({
    steamApiKey: '',
    steamUserId: '',
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
  
  // Debounce timer ref
  const debounceTimerRef = useRef<NodeJS.Timeout | null>(null);

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

  useEffect(() => {
    loadConfig();
    
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
      if (!config.steamApiKey || !config.steamUserId || !config.ludusaviPath || !config.backupPath) {
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
                <p className="font-semibold text-blue-300 text-base mb-2">How to get your Steam User ID:</p>
                <p>1. Go to <a href="https://steamcommunity.com" target="_blank" className="text-blue-400 underline hover:text-blue-300 font-medium">steamcommunity.com</a></p>
                <p>2. Click your profile name → Your Steam64 ID is in the URL</p>
                <p className="mt-2">Example: <span className="text-blue-300 font-mono">steamcommunity.com/profiles/<strong className="text-blue-200">76561198012345678</strong></span></p>
              </div>
            </div>

            {/* Settings Form */}
            <div className="bg-[#1a1f3a] rounded-xl p-8 border border-[#2a3142] shadow-xl space-y-8">
              <div className="flex items-center gap-3 pb-4 border-b border-[#2a3142]">
                <Settings className="w-7 h-7 text-blue-400" />
                <h2 className="text-2xl font-bold text-white">Configuration</h2>
              </div>

              {/* Steam API Key */}
              <div className="space-y-3">
                <label className="block text-sm font-semibold text-gray-200 flex items-center gap-2">
                  <Key className="w-4 h-4 text-blue-400" />
                  Steam Web API Key
                  <span className="text-red-400">*</span>
                </label>
                <input
                  type="password"
                  value={config.steamApiKey}
                  onChange={(e) => setConfig({ ...config, steamApiKey: e.target.value })}
                  placeholder="Enter your Steam API key"
                  className="w-full bg-[#0f1420] border-2 border-[#2a3142] rounded-lg px-4 py-3.5 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-all"
                />
                <p className="text-xs text-gray-400 flex items-center gap-1">
                  Get your API key at: 
                  <a href="https://steamcommunity.com/dev/apikey" target="_blank" className="text-blue-400 hover:text-blue-300 underline font-medium">
                    steamcommunity.com/dev/apikey
                  </a>
                </p>
              </div>

              {/* Steam User ID */}
              <div className="space-y-3">
                <label className="block text-sm font-semibold text-gray-200 flex items-center gap-2">
                  Steam User ID (Steam64)
                  <span className="text-red-400">*</span>
                </label>
                <input
                  type="text"
                  value={config.steamUserId}
                  onChange={(e) => setConfig({ ...config, steamUserId: e.target.value })}
                  placeholder="76561198012345678"
                  className="w-full bg-[#0f1420] border-2 border-[#2a3142] rounded-lg px-4 py-3.5 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-all font-mono"
                />
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
              <div className="pt-6 border-t border-[#2a3142]">
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
      </div>
    </div>
  );
}

export default App;