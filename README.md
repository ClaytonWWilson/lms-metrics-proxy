# LMS Metrics Proxy

A proxy server that tracks and logs token usage for LM Studio API requests.

## Features

- **Transparent Proxying**: Works with existing LM Studio clients without code changes
- **Token Tracking**: Automatically captures input and output token counts for every request
- **Usage Statistics**: Query endpoints to view summary stats, per-model breakdowns, and recent requests
- **Streaming Support**: Handles both standard and streaming (Server-Sent Events) responses
- **Persistent Storage**: SQLite database stores complete request history with metadata
- **Zero-Copy Performance**: Built with Rust for high performance and low overhead

## What It Does

This tool acts as a transparent middleman between your API client and LM Studio:

```
Your Application
    ↓
LMS Metrics Proxy (port 8080)
    ├─→ Logs request details & token usage
    ├─→ Forwards to LM Studio (port 1234)
    ├─→ Stores response metadata in database
    ↓
Returns response to your application
```

**Why use this?**
- Track how many tokens your applications are using
- Monitor API costs and usage patterns
- Audit which models are being called and how often
- Analyze performance metrics (response times, tokens per request)
- Debug API interactions with full request/response logging

## Prerequisites

- **Rust**: Install from [rustup.rs](https://rustup.rs/)
- **Language Model Server**: Any OpenAI-compatible API server (e.g., LM Studio, LocalAI, Ollama with OpenAI compatibility, etc.)
- Your language model server must be running with its API enabled (default port: 1234 for LM Studio)

## Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd token_counter
```

2. Copy the example configuration file:
```bash
cp .env.example .env
```

3. Edit `.env` if needed (defaults work for most setups):
```env
PORT=8080                              # Port for the proxy server
LM_STUDIO_URL=http://localhost:1234    # LM Studio API endpoint (excluding /v1/)
DATABASE_URL=sqlite:./token_counter.db # Database file location
RUST_LOG=info                          # Logging level
```

4. Build and run:
```bash
cargo run --release
```

The server will start and automatically create the SQLite database on first run.

## Configuration

Edit the `.env` file to customize settings:

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | Port the proxy server listens on | `8080` |
| `LM_STUDIO_URL` | Base URL for LM Studio API | `http://localhost:1234` |
| `DATABASE_URL` | SQLite database path | `sqlite:./token_counter.db` |
| `RUST_LOG` | Logging level (trace, debug, info, warn, error) | `info` |

## Usage

### Starting the Server

```bash
cargo run --release
```

You should see output like:
```
Starting token counter proxy on port 8080 with LM Studio at http://localhost:1234
Database initialized at sqlite:./token_counter.db
Proxy server listening on 0.0.0.0:8080
```

### Using with Your Application

Simply point your API client to `http://localhost:8080` instead of `http://localhost:1234`:

**Before (direct to LM Studio):**
```python
import requests

response = requests.post(
    "http://localhost:1234/v1/chat/completions",
    json={
        "model": "llama-3.2-1b-instruct",
        "messages": [{"role": "user", "content": "Hello!"}]
    }
)
```

**After (through Token Counter):**
```python
import requests

response = requests.post(
    "http://localhost:8080/v1/chat/completions",  # Changed port
    json={
        "model": "llama-3.2-1b-instruct",
        "messages": [{"role": "user", "content": "Hello!"}]
    }
)
```

The proxy forwards all `/v1/*` routes to LM Studio and logs the token usage automatically.

### Querying Statistics

Use the statistics endpoints to view usage data:

```bash
# Check server health
curl http://localhost:8080/health

# Get overall usage summary
curl http://localhost:8080/stats/summary

# Get usage breakdown by model
curl http://localhost:8080/stats/by-model

# Get 50 most recent requests
curl http://localhost:8080/stats/recent?limit=50
```

## API Endpoints

### Statistics Endpoints

#### `GET /health`
Health check endpoint.

**Response:**
```json
{
  "status": "ok",
  "service": "token_counter_proxy"
}
```

#### `GET /stats/summary`
Returns overall usage statistics across all models and requests.

**Response:**
```json
{
  "total_requests": 150,
  "successful_requests": 148,
  "failed_requests": 2,
  "total_input_tokens": 12543,
  "total_output_tokens": 45621,
  "total_tokens": 58164,
  "average_input_tokens": 83.6,
  "average_output_tokens": 304.1,
  "total_duration_ms": 125430
}
```

#### `GET /stats/by-model`
Returns usage statistics grouped by model.

**Response:**
```json
{
  "models": [
    {
      "model": "llama-3.2-1b-instruct",
      "request_count": 100,
      "total_input_tokens": 8500,
      "total_output_tokens": 32000,
      "total_tokens": 40500,
      "avg_input_tokens": 85.0,
      "avg_output_tokens": 320.0,
      "avg_duration_ms": 850
    },
    {
      "model": "mistral-7b-instruct",
      "request_count": 50,
      "total_input_tokens": 4043,
      "total_output_tokens": 13621,
      "total_tokens": 17664,
      "avg_input_tokens": 80.9,
      "avg_output_tokens": 272.4,
      "avg_duration_ms": 1200
    }
  ]
}
```

#### `GET /stats/recent?limit=N`
Returns the N most recent requests (max 1000, default 100).

**Parameters:**
- `limit` (optional): Number of requests to return (1-1000, default: 100)

**Response:**
```json
{
  "requests": [
    {
      "id": 150,
      "endpoint": "/v1/chat/completions",
      "model": "llama-3.2-1b-instruct",
      "input_tokens": 85,
      "output_tokens": 320,
      "total_tokens": 405,
      "duration_ms": 850,
      "start_time": "2026-01-19T10:30:45Z",
      "is_error": false,
      "was_streamed": false
    }
  ]
}
```

### Proxy Endpoints

All `/v1/*` routes are automatically forwarded to LM Studio. Supported methods: GET, POST, DELETE.

Common LM Studio endpoints that work through the proxy:
- `POST /v1/chat/completions` - Chat completions (standard & streaming)
- `POST /v1/completions` - Text completions
- `GET /v1/models` - List available models

## Example Workflows

### Basic Usage Flow

1. Start Token Counter and LM Studio
2. Make an API request through the proxy
3. Check the stats to see token usage

```bash
# Terminal 1: Start the proxy
cargo run --release

# Terminal 2: Make a request
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3.2-1b-instruct",
    "messages": [{"role": "user", "content": "Explain Rust in one sentence."}]
  }'

# Terminal 2: Check usage stats
curl http://localhost:8080/stats/summary
```

### Monitoring Token Usage Over Time

```bash
# Get summary before
curl http://localhost:8080/stats/summary > before.json

# Run your application...
python my_app.py

# Get summary after
curl http://localhost:8080/stats/summary > after.json

# Compare to see tokens used
diff before.json after.json
```

### Checking Per-Model Usage

```bash
# See which models are using the most tokens
curl http://localhost:8080/stats/by-model | jq '.models | sort_by(.total_tokens) | reverse'
```

### Viewing Recent Requests

```bash
# Get last 10 requests with formatted output
curl "http://localhost:8080/stats/recent?limit=10" | jq '.requests[] | {model, tokens: .total_tokens, duration_ms}'
```

## Troubleshooting

### Port Already in Use

If port 8080 is already taken:

1. Edit `.env` and change `PORT` to another value (e.g., 8081)
2. Restart the proxy
3. Update your API client to use the new port

### Cannot Connect to LM Studio

**Error:** Connection refused or timeout when making requests

**Solutions:**
- Verify LM Studio is running and the local server is started
- Check LM Studio's server settings (should be on port 1234 by default)
- If LM Studio uses a different port, update `LM_STUDIO_URL` in `.env`
- Try accessing LM Studio directly: `curl http://localhost:1234/v1/models`

### Database Permission Errors

**Error:** Cannot create or write to database file

**Solutions:**
- Ensure the directory for `DATABASE_URL` exists and is writable
- Check file permissions on `token_counter.db`
- Try using an absolute path in `DATABASE_URL` (e.g., `sqlite:/home/user/data/token_counter.db`)

### No Token Counts in Statistics

**Possible causes:**
- Only POST requests with JSON bodies are tracked (GET requests to `/v1/models` won't show token counts)
- LM Studio response might not include `usage` field - check LM Studio settings
- Errors during proxying prevent token logging - check logs with `RUST_LOG=debug`

## Technical Details

### Architecture

- **Framework**: Axum (async web framework)
- **Runtime**: Tokio (async runtime)
- **HTTP Client**: Hyper with TLS support
- **Database**: SQLite with SQLx for async queries
- **Logging**: Tracing for structured logging

### Database Schema

The `requests` table stores:
- Request metadata (endpoint, model, timestamps)
- Token counts (input, output, total)
- Content (prompt and response text)
- Performance metrics (duration_ms)
- Error tracking (is_error, http_status, error_message)
- Streaming flag (was_streamed)

Indexed on: model, endpoint, start_time, is_error for fast queries.

### Streaming Handling

For streaming responses (Server-Sent Events):
1. Proxy spawns a background task to parse SSE chunks
2. Response is forwarded to client in real-time
3. Token counts are extracted as they arrive in `data: [DONE]` frames
4. Database logging happens asynchronously without blocking the stream

## License

MIT License - see LICENSE file for details

