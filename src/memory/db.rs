use chrono::{NaiveDateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMessage {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub consolidated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFact {
    pub id: i64,
    pub fact: String,
    pub importance: f32,
    pub created_at: String,
    pub last_access: String,
    pub access_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeSummary {
    pub id: i64,
    pub level: u32,
    pub summary: String,
    pub created_at: String,
    pub merged: bool,
    pub start_message_id: Option<i64>,
    pub end_message_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryStats {
    pub messages: usize,
    pub unconsolidated_messages: usize,
    pub facts: usize,
    pub episodes: usize,
    pub highest_episode_level: u32,
    pub database_bytes: u64,
    pub wal_bytes: u64,
    pub shm_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryMaintenanceReport {
    pub deleted_messages: usize,
    pub deleted_facts: usize,
    pub deleted_episodes: usize,
    pub checkpointed: bool,
    pub compacted: bool,
    pub database_bytes: u64,
    pub wal_bytes: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct ScoredFact {
    pub fact: SemanticFact,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct ScoredEpisode {
    pub episode: EpisodeSummary,
    pub score: f32,
}

#[derive(Clone)]
pub struct MemoryStore {
    pub character_name: String,
    pub db_path: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

impl MemoryStore {
    pub fn new<P: AsRef<Path>>(db_path: P, character_name: &str) -> Result<Self> {
        if let Some(parent) = db_path.as_ref().parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        }
        let conn = Connection::open(db_path.as_ref())?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            PRAGMA synchronous = NORMAL;
            PRAGMA wal_autocheckpoint = 256;
            PRAGMA journal_size_limit = 4194304;
            PRAGMA temp_store = MEMORY;
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                consolidated INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS facts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                fact TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 1.0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                last_access DATETIME DEFAULT CURRENT_TIMESTAMP,
                access_count INTEGER NOT NULL DEFAULT 0,
                vec_json TEXT NOT NULL DEFAULT '[]',
                source_message_id INTEGER
            );
            CREATE TABLE IF NOT EXISTS episodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                level INTEGER NOT NULL DEFAULT 0,
                summary TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                merged INTEGER NOT NULL DEFAULT 0,
                vec_json TEXT NOT NULL DEFAULT '[]',
                start_message_id INTEGER,
                end_message_id INTEGER
            );
            CREATE TABLE IF NOT EXISTS state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;
        migrate_legacy_schema(&conn)?;
        conn.execute_batch(
            "
            CREATE INDEX IF NOT EXISTS idx_messages_consolidated
                ON messages(consolidated, id);
            CREATE INDEX IF NOT EXISTS idx_episodes_level_merged
                ON episodes(level, merged, id);
            ",
        )?;

        Ok(Self {
            character_name: character_name.to_string(),
            db_path: db_path.as_ref().to_path_buf(),
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn for_character_in_dir<P: AsRef<Path>>(directory: P, name: &str) -> Result<Self> {
        let safe_name = safe_character_file_stem(name);
        let path = directory.as_ref().join(format!("{safe_name}.db"));
        Self::new(path, name)
    }

    #[cfg(test)]
    pub fn add_message(&self, role: &str, content: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        conn.execute(
            "INSERT INTO messages (role, content, consolidated) VALUES (?1, ?2, 0)",
            params![role, content],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn add_turn(&self, user_content: &str, character_content: &str) -> Result<i64> {
        let mut conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let transaction = conn.transaction()?;
        transaction.execute(
            "INSERT INTO messages (role, content, consolidated) VALUES ('user', ?1, 0)",
            params![user_content],
        )?;
        let user_message_id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO messages (role, content, consolidated)
             VALUES ('character', ?1, 0)",
            params![character_content],
        )?;
        transaction.commit()?;
        Ok(user_message_id)
    }

    pub fn get_recent_messages(&self, limit: usize) -> Result<Vec<MemoryMessage>> {
        let conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, role, content, timestamp, consolidated
             FROM messages ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], map_message)?;
        let mut messages = collect_rows(rows)?;
        messages.reverse();
        Ok(messages)
    }

    pub fn get_message_history(
        &self,
        limit: usize,
        before_id: Option<i64>,
    ) -> Result<Vec<MemoryMessage>> {
        let conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, role, content, timestamp, consolidated
             FROM messages
             WHERE (?1 IS NULL OR id < ?1)
             ORDER BY id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            params![before_id, limit as i64],
            map_message,
        )?;
        collect_rows(rows)
    }

    pub fn get_unconsolidated_messages(
        &self,
        limit: usize,
        keep_recent: usize,
    ) -> Result<Vec<MemoryMessage>> {
        let conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, role, content, timestamp, consolidated
             FROM messages
             WHERE consolidated = 0
               AND id NOT IN (
                 SELECT id FROM messages
                 WHERE consolidated = 0
                 ORDER BY id DESC LIMIT ?2
               )
             ORDER BY id ASC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64, keep_recent as i64], map_message)?;
        collect_rows(rows)
    }

    pub fn mark_messages_consolidated(&self, ids: &[i64]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let transaction = conn.transaction()?;
        {
            let mut stmt =
                transaction.prepare("UPDATE messages SET consolidated = 1 WHERE id = ?1")?;
            for id in ids {
                stmt.execute(params![id])?;
            }
        }
        transaction.commit()
    }

    pub fn add_or_reinforce_fact(
        &self,
        fact: &str,
        importance: f32,
        vector: &[f32],
        source_message_id: Option<i64>,
        dedupe_similarity: f32,
    ) -> Result<i64> {
        let vector_json = serde_json::to_string(vector).unwrap_or_else(|_| "[]".to_string());
        let mut conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let transaction = conn.transaction()?;

        let existing = {
            let mut stmt =
                transaction.prepare("SELECT id, fact, vec_json FROM facts ORDER BY id DESC")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            let mut best: Option<(i64, f32)> = None;
            for row in rows {
                let (id, existing_fact, existing_vector_json) = row?;
                let similarity = if normalize_fact(&existing_fact) == normalize_fact(fact) {
                    1.0
                } else {
                    let existing_vector: Vec<f32> =
                        serde_json::from_str(&existing_vector_json).unwrap_or_default();
                    cosine_similarity(&existing_vector, vector)
                };
                if similarity >= dedupe_similarity
                    && best.map(|(_, score)| similarity > score).unwrap_or(true)
                {
                    best = Some((id, similarity));
                }
            }
            best
        };

        let id = if let Some((id, _)) = existing {
            transaction.execute(
                "UPDATE facts
                 SET importance = MIN(10.0, MAX(importance, ?2) + 0.25),
                     last_access = CURRENT_TIMESTAMP,
                     access_count = access_count + 1
                 WHERE id = ?1",
                params![id, importance.clamp(1.0, 10.0)],
            )?;
            id
        } else {
            transaction.execute(
                "INSERT INTO facts
                    (fact, importance, last_access, access_count, vec_json, source_message_id)
                 VALUES (?1, ?2, CURRENT_TIMESTAMP, 0, ?3, ?4)",
                params![
                    fact.trim(),
                    importance.clamp(1.0, 10.0),
                    vector_json,
                    source_message_id
                ],
            )?;
            transaction.last_insert_rowid()
        };
        transaction.commit()?;
        Ok(id)
    }

    pub fn get_facts(&self, limit: usize) -> Result<Vec<SemanticFact>> {
        let conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, fact, importance, created_at, last_access, access_count
             FROM facts ORDER BY importance DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], map_fact)?;
        collect_rows(rows)
    }

    pub(crate) fn retrieve_facts(
        &self,
        query_vector: &[f32],
        limit: usize,
        recency_halflife_days: f32,
    ) -> Result<Vec<ScoredFact>> {
        let conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, fact, importance, created_at, last_access, access_count, vec_json
             FROM facts",
        )?;
        let rows = stmt.query_map([], |row| {
            let vector_json: String = row.get(6)?;
            let vector: Vec<f32> = serde_json::from_str(&vector_json).unwrap_or_default();
            Ok((map_fact(row)?, vector))
        })?;
        let mut scored = Vec::new();
        for row in rows {
            let (fact, vector) = row?;
            let semantic = cosine_similarity(query_vector, &vector).max(0.0);
            let importance = 0.45 + 0.55 * (fact.importance / 10.0).clamp(0.0, 1.0);
            let recency = recency_score(&fact.last_access, recency_halflife_days);
            let score = (semantic + 0.08) * importance * recency;
            scored.push(ScoredFact { fact, score });
        }
        scored.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);
        Ok(scored)
    }

    pub fn touch_facts(&self, ids: &[i64]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let transaction = conn.transaction()?;
        {
            let mut stmt = transaction.prepare(
                "UPDATE facts
                 SET last_access = CURRENT_TIMESTAMP,
                     access_count = access_count + 1
                 WHERE id = ?1",
            )?;
            for id in ids {
                stmt.execute(params![id])?;
            }
        }
        transaction.commit()
    }

    pub fn add_episode(
        &self,
        level: u32,
        summary: &str,
        vector: &[f32],
        start_message_id: Option<i64>,
        end_message_id: Option<i64>,
    ) -> Result<i64> {
        let vector_json = serde_json::to_string(vector).unwrap_or_else(|_| "[]".to_string());
        let conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        conn.execute(
            "INSERT INTO episodes
                (level, summary, merged, vec_json, start_message_id, end_message_id)
             VALUES (?1, ?2, 0, ?3, ?4, ?5)",
            params![
                level as i64,
                summary.trim(),
                vector_json,
                start_message_id,
                end_message_id
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_episodes(&self, limit: usize) -> Result<Vec<EpisodeSummary>> {
        let conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, level, summary, created_at, merged,
                    start_message_id, end_message_id
             FROM episodes ORDER BY level DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], map_episode)?;
        collect_rows(rows)
    }

    pub(crate) fn retrieve_episodes(
        &self,
        query_vector: &[f32],
        limit: usize,
        recency_halflife_days: f32,
    ) -> Result<Vec<ScoredEpisode>> {
        let conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, level, summary, created_at, merged,
                    start_message_id, end_message_id, vec_json
             FROM episodes",
        )?;
        let rows = stmt.query_map([], |row| {
            let vector_json: String = row.get(7)?;
            let vector: Vec<f32> = serde_json::from_str(&vector_json).unwrap_or_default();
            Ok((map_episode(row)?, vector))
        })?;
        let mut scored = Vec::new();
        for row in rows {
            let (episode, vector) = row?;
            let semantic = cosine_similarity(query_vector, &vector).max(0.0);
            let recency = recency_score(&episode.created_at, recency_halflife_days * 2.0);
            let level_weight = 1.0 + (episode.level as f32 * 0.05);
            scored.push(ScoredEpisode {
                episode,
                score: (semantic + 0.05) * recency * level_weight,
            });
        }
        scored.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);
        Ok(scored)
    }

    pub fn get_unmerged_episodes(&self, level: u32, limit: usize) -> Result<Vec<EpisodeSummary>> {
        let conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, level, summary, created_at, merged,
                    start_message_id, end_message_id
             FROM episodes
             WHERE level = ?1 AND merged = 0
             ORDER BY id ASC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![level as i64, limit as i64], map_episode)?;
        collect_rows(rows)
    }

    pub fn mark_episodes_merged(&self, ids: &[i64]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let transaction = conn.transaction()?;
        {
            let mut stmt =
                transaction.prepare("UPDATE episodes SET merged = 1 WHERE id = ?1")?;
            for id in ids {
                stmt.execute(params![id])?;
            }
        }
        transaction.commit()
    }

    pub fn count_unconsolidated_messages(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE consolidated = 0",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value as usize)
    }

    pub fn stats(&self) -> Result<MemoryStats> {
        let conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let messages = count_table(&conn, "messages")?;
        let unconsolidated_messages = conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE consolidated = 0",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let facts = count_table(&conn, "facts")?;
        let episodes = count_table(&conn, "episodes")?;
        let highest_episode_level = conn
            .query_row("SELECT MAX(level) FROM episodes", [], |row| {
                row.get::<_, Option<i64>>(0)
            })
            .optional()?
            .flatten()
            .unwrap_or(0) as u32;
        Ok(MemoryStats {
            messages,
            unconsolidated_messages,
            facts,
            episodes,
            highest_episode_level,
            database_bytes: file_size(&self.db_path),
            wal_bytes: file_size(&adjacent_database_file(&self.db_path, "-wal")),
            shm_bytes: file_size(&adjacent_database_file(&self.db_path, "-shm")),
        })
    }

    pub fn enforce_retention(
        &self,
        max_messages: usize,
        max_facts: usize,
        max_episodes: usize,
        compact: bool,
        checkpoint: bool,
    ) -> Result<MemoryMaintenanceReport> {
        let mut conn = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let transaction = conn.transaction()?;
        let deleted_messages = if max_messages == 0 {
            0
        } else {
            transaction.execute(
                "DELETE FROM messages
                 WHERE consolidated = 1
                   AND id NOT IN (
                     SELECT id FROM messages ORDER BY id DESC LIMIT ?1
                   )",
                params![max_messages as i64],
            )?
        };
        let deleted_facts = if max_facts == 0 {
            0
        } else {
            transaction.execute(
                "DELETE FROM facts
                 WHERE id IN (
                     SELECT id FROM facts
                     ORDER BY importance DESC, last_access DESC, id DESC
                     LIMIT -1 OFFSET ?1
                 )",
                params![max_facts as i64],
            )?
        };
        let deleted_episodes = if max_episodes == 0 {
            0
        } else {
            transaction.execute(
                "DELETE FROM episodes
                 WHERE merged = 1
                   AND id NOT IN (
                     SELECT id FROM episodes ORDER BY id DESC LIMIT ?1
                   )",
                params![max_episodes as i64],
            )?
        };
        transaction.commit()?;

        let changed = deleted_messages + deleted_facts + deleted_episodes > 0;
        if changed || compact || checkpoint {
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        }
        if compact {
            conn.execute_batch("VACUUM;")?;
        }
        drop(conn);

        Ok(MemoryMaintenanceReport {
            deleted_messages,
            deleted_facts,
            deleted_episodes,
            checkpointed: changed || compact || checkpoint,
            compacted: compact,
            database_bytes: file_size(&self.db_path),
            wal_bytes: file_size(&adjacent_database_file(&self.db_path, "-wal")),
        })
    }
}

fn adjacent_database_file(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|metadata| metadata.len()).unwrap_or(0)
}

pub fn safe_character_file_stem(name: &str) -> String {
    let mut safe = String::with_capacity(name.len().min(64));
    for character in name.chars().take(64) {
        if character.is_alphanumeric() || matches!(character, '-' | '_') {
            safe.push(character.to_ascii_lowercase());
        } else {
            safe.push('_');
        }
    }
    let safe = safe.trim_matches('_').to_string();
    if safe.is_empty() {
        "character".to_string()
    } else {
        safe
    }
}

pub(crate) fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.is_empty() || left.len() != right.len() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;
    for (left_value, right_value) in left.iter().zip(right) {
        dot += left_value * right_value;
        left_norm += left_value * left_value;
        right_norm += right_value * right_value;
    }
    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

fn migrate_legacy_schema(conn: &Connection) -> Result<()> {
    ensure_column(
        conn,
        "messages",
        "consolidated",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "facts",
        "last_access",
        "DATETIME DEFAULT CURRENT_TIMESTAMP",
    )?;
    ensure_column(
        conn,
        "facts",
        "access_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(conn, "facts", "vec_json", "TEXT NOT NULL DEFAULT '[]'")?;
    ensure_column(conn, "facts", "source_message_id", "INTEGER")?;
    ensure_column(
        conn,
        "episodes",
        "merged",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "episodes",
        "vec_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    ensure_column(conn, "episodes", "start_message_id", "INTEGER")?;
    ensure_column(conn, "episodes", "end_message_id", "INTEGER")?;
    Ok(())
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for existing in columns {
        if existing? == column {
            return Ok(());
        }
    }
    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        [],
    )?;
    Ok(())
}

fn map_message(row: &rusqlite::Row<'_>) -> Result<MemoryMessage> {
    Ok(MemoryMessage {
        id: row.get(0)?,
        role: row.get(1)?,
        content: row.get(2)?,
        timestamp: row.get(3)?,
        consolidated: row.get::<_, i64>(4)? != 0,
    })
}

fn map_fact(row: &rusqlite::Row<'_>) -> Result<SemanticFact> {
    Ok(SemanticFact {
        id: row.get(0)?,
        fact: row.get(1)?,
        importance: row.get(2)?,
        created_at: row.get(3)?,
        last_access: row.get(4)?,
        access_count: row.get::<_, i64>(5)? as u32,
    })
}

fn map_episode(row: &rusqlite::Row<'_>) -> Result<EpisodeSummary> {
    Ok(EpisodeSummary {
        id: row.get(0)?,
        level: row.get::<_, i64>(1)? as u32,
        summary: row.get(2)?,
        created_at: row.get(3)?,
        merged: row.get::<_, i64>(4)? != 0,
        start_message_id: row.get(5)?,
        end_message_id: row.get(6)?,
    })
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> Result<T>>,
) -> Result<Vec<T>> {
    let mut values = Vec::new();
    for row in rows {
        values.push(row?);
    }
    Ok(values)
}

fn normalize_fact(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace() && !character.is_ascii_punctuation())
        .flat_map(char::to_lowercase)
        .collect()
}

fn recency_score(timestamp: &str, halflife_days: f32) -> f32 {
    if halflife_days <= 0.0 {
        return 1.0;
    }
    let parsed = NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S");
    let Ok(parsed) = parsed else {
        return 1.0;
    };
    let age_seconds = (Utc::now().naive_utc() - parsed).num_seconds().max(0) as f32;
    let age_days = age_seconds / 86_400.0;
    0.5_f32.powf(age_days / halflife_days).max(0.05)
}

fn count_table(conn: &Connection, table: &str) -> Result<usize> {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get::<_, i64>(0)
    })
    .map(|value| value as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("everchara-memory-{label}-{unique}"))
            .join("memory.db")
    }

    #[test]
    fn safe_character_names_cannot_escape_directory() {
        assert_eq!(safe_character_file_stem("../../A B"), "a_b");
        assert_eq!(safe_character_file_stem("莉莉"), "莉莉");
    }

    #[test]
    fn messages_persist_and_preserve_order() {
        let path = temp_db("messages");
        let store = MemoryStore::new(&path, "Test").unwrap();
        store.add_message("user", "first").unwrap();
        store.add_message("character", "second").unwrap();
        let messages = store.get_recent_messages(10).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "first");
        assert_eq!(messages[1].content, "second");
        drop(store);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn completed_turn_is_written_atomically_in_order() {
        let path = temp_db("turn");
        let store = MemoryStore::new(&path, "Test").unwrap();
        let user_id = store.add_turn("question", "answer").unwrap();
        let messages = store.get_recent_messages(10).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].id, user_id);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "character");
        drop(store);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn similar_facts_are_reinforced_instead_of_duplicated() {
        let path = temp_db("facts");
        let store = MemoryStore::new(&path, "Test").unwrap();
        let vector = vec![1.0, 0.0, 0.0];
        store
            .add_or_reinforce_fact("用户喜欢草莓", 6.0, &vector, Some(1), 0.9)
            .unwrap();
        store
            .add_or_reinforce_fact("用户喜欢草莓", 7.0, &vector, Some(2), 0.9)
            .unwrap();
        let facts = store.get_facts(10).unwrap();
        assert_eq!(facts.len(), 1);
        assert!(facts[0].importance > 7.0);
        assert_eq!(facts[0].access_count, 1);
        drop(store);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn retention_prunes_only_old_consolidated_history_and_bounds_indexes() {
        let path = temp_db("retention");
        let store = MemoryStore::new(&path, "Test").unwrap();
        let mut consolidated_ids = Vec::new();
        for index in 0..10 {
            let id = store
                .add_message("user", &format!("message {index}"))
                .unwrap();
            if index < 8 {
                consolidated_ids.push(id);
            }
        }
        store
            .mark_messages_consolidated(&consolidated_ids)
            .unwrap();
        for index in 0..5 {
            store
                .add_or_reinforce_fact(
                    &format!("fact {index}"),
                    index as f32 + 1.0,
                    &[index as f32 + 1.0, 1.0],
                    None,
                    1.1,
                )
                .unwrap();
            store
                .add_episode(
                    0,
                    &format!("episode {index}"),
                    &[1.0, index as f32],
                    None,
                    None,
                )
                .unwrap();
        }
        let episode_ids = store
            .get_episodes(10)
            .unwrap()
            .into_iter()
            .map(|episode| episode.id)
            .collect::<Vec<_>>();
        store.mark_episodes_merged(&episode_ids).unwrap();

        let report = store
            .enforce_retention(4, 2, 2, false, true)
            .unwrap();
        let stats = store.stats().unwrap();
        assert_eq!(report.deleted_messages, 6);
        assert_eq!(stats.messages, 4);
        assert_eq!(stats.unconsolidated_messages, 2);
        assert_eq!(stats.facts, 2);
        assert_eq!(stats.episodes, 2);
        drop(store);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn zero_retention_limits_preserve_all_persistent_memory() {
        let path = temp_db("unlimited-retention");
        let store = MemoryStore::new(&path, "Test").unwrap();
        for index in 0..3 {
            store
                .add_turn(&format!("question {index}"), &format!("answer {index}"))
                .unwrap();
            store
                .add_or_reinforce_fact(
                    &format!("fact {index}"),
                    1.0,
                    &[index as f32 + 1.0],
                    None,
                    1.1,
                )
                .unwrap();
            store
                .add_episode(
                    0,
                    &format!("episode {index}"),
                    &[index as f32 + 1.0],
                    None,
                    None,
                )
                .unwrap();
        }
        let message_ids = store
            .get_recent_messages(20)
            .unwrap()
            .into_iter()
            .map(|message| message.id)
            .collect::<Vec<_>>();
        store.mark_messages_consolidated(&message_ids).unwrap();
        let episode_ids = store
            .get_episodes(20)
            .unwrap()
            .into_iter()
            .map(|episode| episode.id)
            .collect::<Vec<_>>();
        store.mark_episodes_merged(&episode_ids).unwrap();

        let report = store.enforce_retention(0, 0, 0, false, true).unwrap();
        let stats = store.stats().unwrap();
        assert_eq!(report.deleted_messages, 0);
        assert_eq!(report.deleted_facts, 0);
        assert_eq!(report.deleted_episodes, 0);
        assert_eq!(stats.messages, 6);
        assert_eq!(stats.facts, 3);
        assert_eq!(stats.episodes, 3);
        drop(store);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn message_history_pages_backwards_without_reordering_rows() {
        let path = temp_db("message-history");
        let store = MemoryStore::new(&path, "Test").unwrap();
        for index in 0..4 {
            store
                .add_turn(&format!("question {index}"), &format!("answer {index}"))
                .unwrap();
        }
        let newest = store.get_message_history(3, None).unwrap();
        assert_eq!(
            newest.iter().map(|message| message.id).collect::<Vec<_>>(),
            vec![8, 7, 6]
        );
        let older = store
            .get_message_history(3, newest.last().map(|message| message.id))
            .unwrap();
        assert_eq!(
            older.iter().map(|message| message.id).collect::<Vec<_>>(),
            vec![5, 4, 3]
        );
        drop(store);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn legacy_schema_is_migrated_before_indexes_are_created() {
        let path = temp_db("legacy-migration");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "
                CREATE TABLE messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE facts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    fact TEXT NOT NULL,
                    importance REAL NOT NULL DEFAULT 1.0,
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE episodes (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    level INTEGER NOT NULL DEFAULT 0,
                    summary TEXT NOT NULL,
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
                );
                ",
            )
            .unwrap();
        }

        let store = MemoryStore::new(&path, "Legacy").unwrap();
        store.add_message("user", "migration works").unwrap();
        assert_eq!(store.stats().unwrap().messages, 1);
        drop(store);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
