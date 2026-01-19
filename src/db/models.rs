use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestRecord {
    pub endpoint: String,
    pub model: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_ms: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub prompt: String,
    pub output: String,
    pub request_id: Option<String>,
    pub is_error: bool,
    pub error_message: Option<String>,
    pub http_status: i32,
    pub was_streamed: bool,
}

impl RequestRecord {
    pub fn new(
        endpoint: String,
        model: String,
        start_time: DateTime<Utc>,
        prompt: String,
    ) -> Self {
        Self {
            endpoint,
            model,
            start_time: start_time.to_rfc3339(),
            end_time: String::new(),
            duration_ms: 0,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            prompt,
            output: String::new(),
            request_id: None,
            is_error: false,
            error_message: None,
            http_status: 200,
            was_streamed: false,
        }
    }

    pub fn complete(
        &mut self,
        end_time: DateTime<Utc>,
        output: String,
        input_tokens: i64,
        output_tokens: i64,
        http_status: i32,
        was_streamed: bool,
    ) {
        self.end_time = end_time.to_rfc3339();
        self.output = output;
        self.input_tokens = input_tokens;
        self.output_tokens = output_tokens;
        self.total_tokens = input_tokens + output_tokens;
        self.http_status = http_status;
        self.was_streamed = was_streamed;

        // Calculate duration
        if let (Ok(start), Ok(end)) = (
            DateTime::parse_from_rfc3339(&self.start_time),
            DateTime::parse_from_rfc3339(&self.end_time),
        ) {
            self.duration_ms = (end.timestamp_millis() - start.timestamp_millis()).max(0);
        }
    }

    pub fn set_error(&mut self, end_time: DateTime<Utc>, error_message: String, http_status: i32) {
        self.end_time = end_time.to_rfc3339();
        self.is_error = true;
        self.error_message = Some(error_message);
        self.http_status = http_status;

        // Calculate duration
        if let (Ok(start), Ok(end)) = (
            DateTime::parse_from_rfc3339(&self.start_time),
            DateTime::parse_from_rfc3339(&self.end_time),
        ) {
            self.duration_ms = (end.timestamp_millis() - start.timestamp_millis()).max(0);
        }
    }
}

pub async fn init_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let schema = include_str!("schema.sql");
    sqlx::raw_sql(schema).execute(pool).await?;
    Ok(())
}

pub async fn insert_request(pool: &SqlitePool, record: &RequestRecord) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO requests (
            endpoint, model, start_time, end_time, duration_ms,
            input_tokens, output_tokens, total_tokens,
            prompt, output, request_id, is_error, error_message,
            http_status, was_streamed
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&record.endpoint)
    .bind(&record.model)
    .bind(&record.start_time)
    .bind(&record.end_time)
    .bind(record.duration_ms)
    .bind(record.input_tokens)
    .bind(record.output_tokens)
    .bind(record.total_tokens)
    .bind(&record.prompt)
    .bind(&record.output)
    .bind(&record.request_id)
    .bind(record.is_error)
    .bind(&record.error_message)
    .bind(record.http_status)
    .bind(record.was_streamed)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

#[derive(Debug, Serialize)]
pub struct SummaryStats {
    pub total_requests: i64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_tokens: i64,
    pub avg_input_tokens: f64,
    pub avg_output_tokens: f64,
    pub avg_duration_ms: f64,
}

pub async fn get_summary_stats(pool: &SqlitePool) -> Result<SummaryStats, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) as total_requests,
            SUM(CASE WHEN is_error = 0 THEN 1 ELSE 0 END) as successful_requests,
            SUM(CASE WHEN is_error = 1 THEN 1 ELSE 0 END) as failed_requests,
            COALESCE(SUM(input_tokens), 0) as total_input_tokens,
            COALESCE(SUM(output_tokens), 0) as total_output_tokens,
            COALESCE(SUM(total_tokens), 0) as total_tokens,
            COALESCE(AVG(CAST(input_tokens AS REAL)), 0.0) as avg_input_tokens,
            COALESCE(AVG(CAST(output_tokens AS REAL)), 0.0) as avg_output_tokens,
            COALESCE(AVG(CAST(duration_ms AS REAL)), 0.0) as avg_duration_ms
        FROM requests
        "#
    )
    .fetch_one(pool)
    .await?;

    Ok(SummaryStats {
        total_requests: row.try_get("total_requests")?,
        successful_requests: row.try_get("successful_requests")?,
        failed_requests: row.try_get("failed_requests")?,
        total_input_tokens: row.try_get("total_input_tokens")?,
        total_output_tokens: row.try_get("total_output_tokens")?,
        total_tokens: row.try_get("total_tokens")?,
        avg_input_tokens: row.try_get("avg_input_tokens")?,
        avg_output_tokens: row.try_get("avg_output_tokens")?,
        avg_duration_ms: row.try_get("avg_duration_ms")?,
    })
}

#[derive(Debug, Serialize)]
pub struct ModelStats {
    pub model: String,
    pub requests: i64,
    pub total_tokens: i64,
    pub avg_tokens_per_request: f64,
}

pub async fn get_model_stats(pool: &SqlitePool) -> Result<Vec<ModelStats>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            model,
            COUNT(*) as requests,
            COALESCE(SUM(total_tokens), 0) as total_tokens,
            COALESCE(AVG(CAST(total_tokens AS REAL)), 0.0) as avg_tokens_per_request
        FROM requests
        WHERE is_error = 0
        GROUP BY model
        ORDER BY requests DESC
        "#
    )
    .fetch_all(pool)
    .await?;

    let mut stats = Vec::new();
    for row in rows {
        stats.push(ModelStats {
            model: row.try_get("model")?,
            requests: row.try_get("requests")?,
            total_tokens: row.try_get("total_tokens")?,
            avg_tokens_per_request: row.try_get("avg_tokens_per_request")?,
        });
    }

    Ok(stats)
}

#[derive(Debug, Serialize)]
pub struct RecentRequest {
    pub id: i64,
    pub endpoint: String,
    pub model: String,
    pub start_time: String,
    pub duration_ms: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub is_error: bool,
}

pub async fn get_recent_requests(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<RecentRequest>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            id,
            endpoint,
            model,
            start_time,
            duration_ms,
            input_tokens,
            output_tokens,
            is_error
        FROM requests
        ORDER BY id DESC
        LIMIT ?
        "#
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut requests = Vec::new();
    for row in rows {
        requests.push(RecentRequest {
            id: row.try_get("id")?,
            endpoint: row.try_get("endpoint")?,
            model: row.try_get("model")?,
            start_time: row.try_get("start_time")?,
            duration_ms: row.try_get("duration_ms")?,
            input_tokens: row.try_get("input_tokens")?,
            output_tokens: row.try_get("output_tokens")?,
            is_error: row.try_get("is_error")?,
        });
    }

    Ok(requests)
}
