# LMS Metrics Proxy

A proxy server that tracks and logs token usage for LM Studio API requests.

## Introduction

LMS Metrics Proxy is a transparent middleman between your API client and LM Studio that automatically tracks token usage and provides detailed usage statistics.

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

## Installation and Use

### Method 1: Download from Releases

Download pre-built binaries from the [releases page](https://github.com/ClaytonWWilson/lms-metrics-proxy/releases).

**Available platforms:**

- Linux x86_64
- Windows x86_64
- macOS ARM64

**Linux/macOS:**

```bash
# Download the appropriate binary for your platform
# Make it executable
chmod +x lms_metrics_proxy-VERSION-PLATFORM

# Run the proxy
./lms_metrics_proxy-VERSION-PLATFORM
```

**Windows:**

```powershell
# Download the .exe file
# Run the proxy
.\lms_metrics_proxy-VERSION-windows-x86_64.exe
```

### Method 2: Docker

Run directly from the Docker image:

```bash
docker run -d \
  --name lms-metrics-proxy \
  -p 8080:8080 \
  -v $(pwd)/data:/app/data \
  -e PORT=8080 \
  -e LM_STUDIO_URL=http://host.docker.internal:1234 \
  -e DATABASE_URL=sqlite:./data/metrics.db \
  -e RUST_LOG=info \
  ghcr.io/claytonwwilson/lms-metrics-proxy:latest
```

**Key points:**

- Port 8080 is exposed for the proxy
- Volume mount at `./data` provides persistent database storage
- `host.docker.internal` allows Docker to connect to LM Studio on your host machine
- Environment variables configure the proxy behavior

### Method 3: Docker Compose

The repository includes a [docker-compose.yml](docker-compose.yml) file for easy deployment:

```bash
docker-compose up -d
```

This configuration:

- Uses the same Docker image from GitHub Container Registry
- Maps port 8080 for the proxy
- Creates a `./data` directory for persistent database storage
- Sets `LM_STUDIO_URL` to `http://host.docker.internal:1234` for Docker Desktop
- Automatically restarts the container unless stopped

### Quick Usage Example

Once running, point your API client to `http://localhost:8080` instead of `http://localhost:1234`:

**Python example:**

```python
import requests

response = requests.post(
    "http://localhost:8080/v1/chat/completions",  # Changed from :1234 to :8080
    json={
        "model": "llama-3.2-1b-instruct",
        "messages": [{"role": "user", "content": "Hello!"}]
    }
)
```

The proxy forwards all `/v1/*` routes to LM Studio and logs token usage automatically.

## Configuration

All methods can be configured using environment variables:

| Variable        | Description                                     | Default                 |
| --------------- | ----------------------------------------------- | ----------------------- |
| `PORT`          | Port the proxy server listens on                | `8080`                  |
| `LM_STUDIO_URL` | Base URL for LM Studio API                      | `http://localhost:1234` |
| `DATABASE_URL`  | SQLite database path                            | `sqlite:./metrics.db`   |
| `RUST_LOG`      | Logging level (trace, debug, info, warn, error) | `info`                  |

**For Docker:** Pass environment variables using `-e` flags in the `docker run` command.

**For Docker Compose:** Edit the `environment` section in [docker-compose.yml](docker-compose.yml).

**For binary releases:** Create a `.env` file in the same directory as the binary (see [.env.example](.env.example)).

## API Endpoints

### Statistics Endpoints

#### `GET /health`

Health check endpoint.

**Response:**

```json
{
  "status": "ok",
  "service": "lms_metrics_proxy_proxy"
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
      "requests": 100,
      "input_tokens": 8500,
      "output_tokens": 32000,
      "total_tokens": 40500,
      "avg_tokens_per_request": 405.0
    },
    {
      "model": "mistral-7b-instruct",
      "requests": 50,
      "input_tokens": 4043,
      "output_tokens": 13621,
      "total_tokens": 17664,
      "avg_tokens_per_request": 353.3
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

## License

MIT License - see LICENSE file for details
