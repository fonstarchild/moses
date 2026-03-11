use rusqlite::{Connection, params};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFact {
    pub key: String,
    pub value: String,
    pub category: String,
}

pub struct LongTermMemory {
    conn: Connection,
}

impl LongTermMemory {
    pub fn open(workspace_root: &str) -> Result<Self> {
        std::fs::create_dir_all(format!("{}/.moses", workspace_root))?;
        let path = format!("{}/.moses/memory.db", workspace_root);
        let conn = Connection::open(&path)?;

        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS facts (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT 'general',
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );
            CREATE TABLE IF NOT EXISTS summaries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                scope TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );
            CREATE INDEX IF NOT EXISTS idx_facts_cat ON facts(category);
            CREATE INDEX IF NOT EXISTS idx_summaries_scope ON summaries(scope);
        ")?;

        Ok(Self { conn })
    }

    pub fn store_fact(&self, key: &str, value: &str, category: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO facts (key, value, category, updated_at)
             VALUES (?1, ?2, ?3, strftime('%s','now'))",
            params![key, value, category],
        )?;
        Ok(())
    }

    pub fn get_fact(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT value FROM facts WHERE key = ?1"
        )?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn facts_by_category(&self, category: &str) -> Result<Vec<ProjectFact>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, value, category FROM facts WHERE category = ?1 ORDER BY updated_at DESC LIMIT 50"
        )?;
        let facts = stmt.query_map(params![category], |row| {
            Ok(ProjectFact {
                key: row.get(0)?,
                value: row.get(1)?,
                category: row.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
        Ok(facts)
    }

    pub fn store_summary(&self, scope: &str, content: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO summaries (scope, content) VALUES (?1, ?2)",
            params![scope, content],
        )?;
        Ok(())
    }

    pub fn get_latest_summary(&self, scope: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT content FROM summaries WHERE scope = ?1
             ORDER BY created_at DESC LIMIT 1"
        )?;
        let mut rows = stmt.query(params![scope])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    /// Build a context string from stored project knowledge.
    pub fn project_context_snippet(&self) -> String {
        let mut parts = vec![];

        if let Ok(facts) = self.facts_by_category("architecture") {
            if !facts.is_empty() {
                parts.push("### Project Architecture\n".to_string());
                for f in facts.iter().take(10) {
                    parts.push(format!("- {}: {}", f.key, f.value));
                }
            }
        }

        if let Ok(Some(summary)) = self.get_latest_summary("codebase") {
            parts.push(format!("\n### Codebase Summary\n{}", summary));
        }

        parts.join("\n")
    }
}
