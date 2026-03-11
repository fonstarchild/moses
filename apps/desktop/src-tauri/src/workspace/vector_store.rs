use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    pub file: String,
    pub line: usize,
    pub text: String,
    pub node_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file: String,
    pub line: usize,
    pub snippet: String,
    pub score: f32,
}

pub struct VectorStore {
    conn: Connection,
}

impl VectorStore {
    pub fn open(workspace_root: &str) -> Result<Self> {
        std::fs::create_dir_all(format!("{}/.moses", workspace_root))?;
        let path = format!("{}/.moses/index.db", workspace_root);
        let conn = Connection::open(&path)?;

        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file TEXT NOT NULL,
                line INTEGER NOT NULL,
                node_kind TEXT NOT NULL,
                text TEXT NOT NULL
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts
                USING fts5(text, file, content='chunks', content_rowid='id');
            CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
                INSERT INTO chunks_fts(rowid, text, file) VALUES (new.id, new.text, new.file);
            END;
            CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
                INSERT INTO chunks_fts(chunks_fts, rowid, text, file) VALUES('delete', old.id, old.text, old.file);
            END;
            CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file);
        ")?;

        Ok(Self { conn })
    }

    pub fn insert_text(&self, chunk: &CodeChunk) -> Result<()> {
        self.conn.execute(
            "INSERT INTO chunks (file, line, node_kind, text) VALUES (?1, ?2, ?3, ?4)",
            params![chunk.file, chunk.line as i64, chunk.node_kind, chunk.text],
        )?;
        Ok(())
    }

    pub fn search(&self, query: &str, top_k: usize) -> Result<Vec<SearchResult>> {
        // FTS5 search with BM25 ranking
        let mut stmt = self.conn.prepare(
            "SELECT c.file, c.line, c.text, bm25(chunks_fts) as score
             FROM chunks_fts
             JOIN chunks c ON c.id = chunks_fts.rowid
             WHERE chunks_fts MATCH ?1
             ORDER BY score
             LIMIT ?2",
        )?;

        let results = stmt
            .query_map(params![sanitize_fts_query(query), top_k as i64], |row| {
                Ok(SearchResult {
                    file: row.get(0)?,
                    line: row.get::<_, i64>(1)? as usize,
                    snippet: row.get::<_, String>(2)?.chars().take(300).collect(),
                    score: row.get::<_, f64>(3)? as f32,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    pub fn grep(&self, pattern: &str, top_k: usize) -> Result<Vec<SearchResult>> {
        let like = format!("%{}%", pattern.replace('%', "\\%").replace('_', "\\_"));
        let mut stmt = self.conn.prepare(
            "SELECT file, line, text FROM chunks WHERE text LIKE ?1 ESCAPE '\\' LIMIT ?2",
        )?;

        let results = stmt
            .query_map(params![like, top_k as i64], |row| {
                Ok(SearchResult {
                    file: row.get(0)?,
                    line: row.get::<_, i64>(1)? as usize,
                    snippet: row.get::<_, String>(2)?.chars().take(300).collect(),
                    score: 1.0,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    pub fn clear_file(&self, file: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM chunks WHERE file = ?1", params![file])?;
        Ok(())
    }

    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute("DELETE FROM chunks", [])?;
        Ok(())
    }

    pub fn stats(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
        Ok(count as usize)
    }
}

/// Sanitize a query string for FTS5 — escape special chars
fn sanitize_fts_query(q: &str) -> String {
    // Wrap each word in quotes for FTS5 phrase matching
    let words: Vec<String> = q
        .split_whitespace()
        .map(|w| format!("\"{}\"", w.replace('"', "")))
        .collect();
    words.join(" OR ")
}
