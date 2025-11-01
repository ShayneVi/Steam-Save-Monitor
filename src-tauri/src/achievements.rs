use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use rusqlite::{Connection, params};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    pub id: Option<i64>,
    pub app_id: u32,
    pub game_name: String,
    pub achievement_id: String,
    pub display_name: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub icon_gray_url: Option<String>,
    pub hidden: bool,
    pub achieved: bool,
    pub unlock_time: Option<i64>,
    pub source: String, // "Steam", "Goldberg", "CODEX", etc.
    pub last_updated: i64,
    pub global_unlock_percentage: Option<f32>, // Global unlock percentage from Steam API
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameAchievementSummary {
    pub app_id: u32,
    pub game_name: String,
    pub total_achievements: i32,
    pub unlocked_achievements: i32,
    pub source: String,
    pub last_updated: i64,
}

pub struct AchievementDatabase {
    conn: Connection,
}

impl AchievementDatabase {
    pub fn new(db_path: PathBuf) -> Result<Self, String> {
        let conn = Connection::open(db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let db = AchievementDatabase { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<(), String> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS achievements (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                app_id INTEGER NOT NULL,
                game_name TEXT NOT NULL,
                achievement_id TEXT NOT NULL,
                display_name TEXT NOT NULL,
                description TEXT,
                icon_url TEXT,
                icon_gray_url TEXT,
                hidden INTEGER DEFAULT 0,
                achieved INTEGER DEFAULT 0,
                unlock_time INTEGER,
                source TEXT NOT NULL,
                last_updated INTEGER NOT NULL,
                global_unlock_percentage REAL,
                UNIQUE(app_id, achievement_id, source)
            )",
            [],
        ).map_err(|e| format!("Failed to create achievements table: {}", e))?;

        // Add column if it doesn't exist (for existing databases)
        let _ = self.conn.execute(
            "ALTER TABLE achievements ADD COLUMN global_unlock_percentage REAL",
            [],
        );

        // Create index for faster queries
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_app_id ON achievements(app_id)",
            [],
        ).map_err(|e| format!("Failed to create index: {}", e))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_achieved ON achievements(achieved)",
            [],
        ).map_err(|e| format!("Failed to create index: {}", e))?;

        Ok(())
    }

    pub fn insert_or_update_achievement(&self, achievement: &Achievement) -> Result<(), String> {
        self.conn.execute(
            "INSERT INTO achievements (
                app_id, game_name, achievement_id, display_name, description,
                icon_url, icon_gray_url, hidden, achieved, unlock_time, source, last_updated, global_unlock_percentage
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(app_id, achievement_id, source) DO UPDATE SET
                display_name = excluded.display_name,
                description = excluded.description,
                icon_url = excluded.icon_url,
                icon_gray_url = excluded.icon_gray_url,
                hidden = excluded.hidden,
                achieved = excluded.achieved,
                unlock_time = excluded.unlock_time,
                last_updated = excluded.last_updated,
                global_unlock_percentage = excluded.global_unlock_percentage",
            params![
                achievement.app_id,
                achievement.game_name,
                achievement.achievement_id,
                achievement.display_name,
                achievement.description,
                achievement.icon_url,
                achievement.icon_gray_url,
                achievement.hidden as i32,
                achievement.achieved as i32,
                achievement.unlock_time,
                achievement.source,
                achievement.last_updated,
                achievement.global_unlock_percentage,
            ],
        ).map_err(|e| format!("Failed to insert/update achievement: {}", e))?;

        Ok(())
    }

    pub fn get_game_achievements(&self, app_id: u32) -> Result<Vec<Achievement>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT id, app_id, game_name, achievement_id, display_name, description,
                    icon_url, icon_gray_url, hidden, achieved, unlock_time, source, last_updated, global_unlock_percentage
             FROM achievements WHERE app_id = ?1
             ORDER BY achievement_id"
        ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let achievements = stmt.query_map([app_id], |row| {
            Ok(Achievement {
                id: row.get(0)?,
                app_id: row.get(1)?,
                game_name: row.get(2)?,
                achievement_id: row.get(3)?,
                display_name: row.get(4)?,
                description: row.get(5)?,
                icon_url: row.get(6)?,
                icon_gray_url: row.get(7)?,
                hidden: row.get::<_, i32>(8)? != 0,
                achieved: row.get::<_, i32>(9)? != 0,
                unlock_time: row.get(10)?,
                source: row.get(11)?,
                last_updated: row.get(12)?,
                global_unlock_percentage: row.get(13)?,
            })
        }).map_err(|e| format!("Failed to query achievements: {}", e))?;

        achievements.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect achievements: {}", e))
    }

    pub fn get_all_games(&self) -> Result<Vec<GameAchievementSummary>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT app_id, game_name, source,
                    COUNT(*) as total,
                    SUM(CASE WHEN achieved = 1 THEN 1 ELSE 0 END) as unlocked,
                    MAX(last_updated) as last_updated
             FROM achievements
             GROUP BY app_id, source
             ORDER BY game_name"
        ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let games = stmt.query_map([], |row| {
            Ok(GameAchievementSummary {
                app_id: row.get(0)?,
                game_name: row.get(1)?,
                source: row.get(2)?,
                total_achievements: row.get(3)?,
                unlocked_achievements: row.get(4)?,
                last_updated: row.get(5)?,
            })
        }).map_err(|e| format!("Failed to query games: {}", e))?;

        games.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect games: {}", e))
    }

    pub fn export_to_json(&self) -> Result<String, String> {
        let games = self.get_all_games()?;
        let mut export_data = Vec::new();

        for game in games {
            let achievements = self.get_game_achievements(game.app_id)?;
            export_data.push(serde_json::json!({
                "game": game,
                "achievements": achievements
            }));
        }

        serde_json::to_string_pretty(&export_data)
            .map_err(|e| format!("Failed to serialize to JSON: {}", e))
    }

    pub fn delete_game_achievements(&self, app_id: u32) -> Result<(), String> {
        self.conn.execute(
            "DELETE FROM achievements WHERE app_id = ?1",
            [app_id],
        ).map_err(|e| format!("Failed to delete achievements: {}", e))?;
        Ok(())
    }

    pub fn update_achievement_status(&self, id: i64, achieved: bool, unlock_time: Option<i64>) -> Result<(), String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "UPDATE achievements SET achieved = ?1, unlock_time = ?2, last_updated = ?3 WHERE id = ?4",
            params![achieved as i32, unlock_time, now, id],
        ).map_err(|e| format!("Failed to update achievement status: {}", e))?;

        Ok(())
    }
}
