use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyRecord {
    pub id: String,
    pub key_hash: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub created_at: String,
    pub is_revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestTraceRecord {
    pub id: String,
    pub timestamp: String,
    pub endpoint: String,
    pub model_requested: String,
    pub model_used: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub latency_ms: u64,
    pub status_code: u16,
    pub client_ip: String,
    pub trace_data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: String,
    pub category: String,
    pub key: String,
    pub content: String,
    pub provenance: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: String,
    pub job_type: String,
    pub payload: String,
    pub status: String, // "pending", "running", "completed", "failed"
    pub result: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub id: String,
    pub filename: String,
    pub file_type: String,
    pub size_bytes: u64,
    pub metadata: String,
    pub created_at: String,
}

impl Database {
    pub fn init<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;

            CREATE TABLE IF NOT EXISTS api_keys (
                id TEXT PRIMARY KEY,
                key_hash TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                permissions TEXT NOT NULL,
                created_at TEXT NOT NULL,
                is_revoked INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS request_traces (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                endpoint TEXT NOT NULL,
                model_requested TEXT NOT NULL,
                model_used TEXT NOT NULL,
                prompt_tokens INTEGER NOT NULL,
                completion_tokens INTEGER NOT NULL,
                latency_ms INTEGER NOT NULL,
                status_code INTEGER NOT NULL,
                client_ip TEXT NOT NULL,
                trace_data TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS memory_entries (
                id TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                key TEXT NOT NULL,
                content TEXT NOT NULL,
                provenance TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(category, key)
            );

            CREATE TABLE IF NOT EXISTS audit_logs (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                action TEXT NOT NULL,
                actor TEXT NOT NULL,
                details TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                job_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                status TEXT NOT NULL,
                result TEXT,
                error TEXT,
                created_at TEXT NOT NULL,
                finished_at TEXT
            );

            CREATE TABLE IF NOT EXISTS artifacts (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                file_type TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                metadata TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn record_request(&self, trace: &RequestTraceRecord) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"INSERT INTO request_traces 
               (id, timestamp, endpoint, model_requested, model_used, prompt_tokens, completion_tokens, latency_ms, status_code, client_ip, trace_data)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
            params![
                trace.id,
                trace.timestamp,
                trace.endpoint,
                trace.model_requested,
                trace.model_used,
                trace.prompt_tokens,
                trace.completion_tokens,
                trace.latency_ms,
                trace.status_code,
                trace.client_ip,
                trace.trace_data,
            ],
        )?;
        Ok(())
    }

    pub fn get_recent_requests(&self, limit: usize) -> anyhow::Result<Vec<RequestTraceRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, endpoint, model_requested, model_used, prompt_tokens, completion_tokens, latency_ms, status_code, client_ip, trace_data 
             FROM request_traces ORDER BY timestamp DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(RequestTraceRecord {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                endpoint: row.get(2)?,
                model_requested: row.get(3)?,
                model_used: row.get(4)?,
                prompt_tokens: row.get(5)?,
                completion_tokens: row.get(6)?,
                latency_ms: row.get(7)?,
                status_code: row.get(8)?,
                client_ip: row.get(9)?,
                trace_data: row.get(10)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn insert_api_key(&self, name: &str, raw_key: &str, permissions: &[String]) -> anyhow::Result<ApiKeyRecord> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(raw_key.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let perms_json = serde_json::to_string(permissions)?;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO api_keys (id, key_hash, name, permissions, created_at, is_revoked) VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            params![id, hash, name, perms_json, now],
        )?;

        Ok(ApiKeyRecord {
            id,
            key_hash: hash,
            name: name.to_string(),
            permissions: permissions.to_vec(),
            created_at: now,
            is_revoked: false,
        })
    }

    pub fn verify_api_key(&self, raw_key: &str) -> anyhow::Result<Option<ApiKeyRecord>> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(raw_key.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, key_hash, name, permissions, created_at, is_revoked FROM api_keys WHERE key_hash = ?1 AND is_revoked = 0")?;
        let mut rows = stmt.query(params![hash])?;

        if let Some(row) = rows.next()? {
            let perms_json: String = row.get(3)?;
            let perms = serde_json::from_str(&perms_json).unwrap_or_default();
            Ok(Some(ApiKeyRecord {
                id: row.get(0)?,
                key_hash: row.get(1)?,
                name: row.get(2)?,
                permissions: perms,
                created_at: row.get(4)?,
                is_revoked: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_api_keys(&self) -> anyhow::Result<Vec<ApiKeyRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, key_hash, name, permissions, created_at, is_revoked FROM api_keys ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            let perms_json: String = row.get(3)?;
            let perms = serde_json::from_str(&perms_json).unwrap_or_default();
            Ok(ApiKeyRecord {
                id: row.get(0)?,
                key_hash: row.get(1)?,
                name: row.get(2)?,
                permissions: perms,
                created_at: row.get(4)?,
                is_revoked: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn revoke_api_key(&self, id: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("UPDATE api_keys SET is_revoked = 1 WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    // Persistent Memory
    pub fn upsert_memory(&self, category: &str, key: &str, content: &str, provenance: &str) -> anyhow::Result<MemoryRecord> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"INSERT INTO memory_entries (id, category, key, content, provenance, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
               ON CONFLICT(category, key) DO UPDATE SET
               content = excluded.content,
               provenance = excluded.provenance,
               updated_at = excluded.updated_at"#,
            params![id, category, key, content, provenance, now, now],
        )?;

        Ok(MemoryRecord {
            id,
            category: category.to_string(),
            key: key.to_string(),
            content: content.to_string(),
            provenance: provenance.to_string(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn search_memory(&self, query: &str, category: Option<&str>) -> anyhow::Result<Vec<MemoryRecord>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{}%", query);
        let mut out = Vec::new();

        if let Some(cat) = category {
            let mut stmt = conn.prepare(
                "SELECT id, category, key, content, provenance, created_at, updated_at 
                 FROM memory_entries 
                 WHERE category = ?1 AND (key LIKE ?2 OR content LIKE ?2) 
                 ORDER BY updated_at DESC",
            )?;
            let rows = stmt.query_map(params![cat, pattern], |row| {
                Ok(MemoryRecord {
                    id: row.get(0)?,
                    category: row.get(1)?,
                    key: row.get(2)?,
                    content: row.get(3)?,
                    provenance: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            for r in rows {
                out.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, category, key, content, provenance, created_at, updated_at 
                 FROM memory_entries 
                 WHERE key LIKE ?1 OR content LIKE ?1 
                 ORDER BY updated_at DESC",
            )?;
            let rows = stmt.query_map(params![pattern], |row| {
                Ok(MemoryRecord {
                    id: row.get(0)?,
                    category: row.get(1)?,
                    key: row.get(2)?,
                    content: row.get(3)?,
                    provenance: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            for r in rows {
                out.push(r?);
            }
        }
        Ok(out)
    }

    pub fn list_memory(&self, limit: usize) -> anyhow::Result<Vec<MemoryRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, category, key, content, provenance, created_at, updated_at 
             FROM memory_entries ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(MemoryRecord {
                id: row.get(0)?,
                category: row.get(1)?,
                key: row.get(2)?,
                content: row.get(3)?,
                provenance: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn delete_memory(&self, id: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM memory_entries WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    // Jobs
    pub fn create_job(&self, job_type: &str, payload: &str) -> anyhow::Result<JobRecord> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO jobs (id, job_type, payload, status, created_at) VALUES (?1, ?2, ?3, 'pending', ?4)",
            params![id, job_type, payload, now],
        )?;

        Ok(JobRecord {
            id,
            job_type: job_type.to_string(),
            payload: payload.to_string(),
            status: "pending".to_string(),
            result: None,
            error: None,
            created_at: now,
            finished_at: None,
        })
    }

    pub fn update_job(&self, id: &str, status: &str, result: Option<&str>, error: Option<&str>) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE jobs SET status = ?1, result = ?2, error = ?3, finished_at = ?4 WHERE id = ?5",
            params![status, result, error, now, id],
        )?;
        Ok(())
    }

    pub fn get_job(&self, id: &str) -> anyhow::Result<Option<JobRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, job_type, payload, status, result, error, created_at, finished_at FROM jobs WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(JobRecord {
                id: row.get(0)?,
                job_type: row.get(1)?,
                payload: row.get(2)?,
                status: row.get(3)?,
                result: row.get(4)?,
                error: row.get(5)?,
                created_at: row.get(6)?,
                finished_at: row.get(7)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_jobs(&self, limit: usize) -> anyhow::Result<Vec<JobRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, job_type, payload, status, result, error, created_at, finished_at FROM jobs ORDER BY created_at DESC LIMIT ?1")?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(JobRecord {
                id: row.get(0)?,
                job_type: row.get(1)?,
                payload: row.get(2)?,
                status: row.get(3)?,
                result: row.get(4)?,
                error: row.get(5)?,
                created_at: row.get(6)?,
                finished_at: row.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // Artifacts / Files
    pub fn save_artifact(&self, id: &str, filename: &str, file_type: &str, size_bytes: u64, metadata: &str) -> anyhow::Result<ArtifactRecord> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO artifacts (id, filename, file_type, size_bytes, metadata, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, filename, file_type, size_bytes as i64, metadata, now],
        )?;

        Ok(ArtifactRecord {
            id: id.to_string(),
            filename: filename.to_string(),
            file_type: file_type.to_string(),
            size_bytes,
            metadata: metadata.to_string(),
            created_at: now,
        })
    }

    pub fn get_artifact(&self, id: &str) -> anyhow::Result<Option<ArtifactRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, filename, file_type, size_bytes, metadata, created_at FROM artifacts WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ArtifactRecord {
                id: row.get(0)?,
                filename: row.get(1)?,
                file_type: row.get(2)?,
                size_bytes: row.get::<_, i64>(3)? as u64,
                metadata: row.get(4)?,
                created_at: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_artifacts(&self) -> anyhow::Result<Vec<ArtifactRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, filename, file_type, size_bytes, metadata, created_at FROM artifacts ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(ArtifactRecord {
                id: row.get(0)?,
                filename: row.get(1)?,
                file_type: row.get(2)?,
                size_bytes: row.get::<_, i64>(3)? as u64,
                metadata: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn delete_artifact(&self, id: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM artifacts WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    // Audit log
    pub fn audit_log(&self, action: &str, actor: &str, details: &str) -> anyhow::Result<()> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO audit_logs (id, timestamp, action, actor, details) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, now, action, actor, details],
        )?;
        Ok(())
    }
}
