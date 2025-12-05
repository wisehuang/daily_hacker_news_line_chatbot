# Daily Hacker News Bot

A Rust application that fetches daily Hacker News stories, provides AI-generated summaries, and sends rich interactive updates via LINE Messaging API. The bot uses LINE Flex Messages to create beautiful carousel cards with summaries and respond to user queries.

## Features

- **Rich Story Cards**: Display stories in swipeable carousel format with LINE Flex Messages
- **AI-Powered Summaries**: Generate concise 2-3 sentence summaries for each story using ChatGPT
- **Combined Story View**: Each carousel card shows story rank, title, summary, and read button
- **Concurrent Processing**: Generate multiple story summaries in parallel for fast response
- **Smart Broadcast**: Send formatted daily summaries with visual hierarchy and separators
- **Interactive Responses**: Respond to user queries and commands via LINE webhook
- **URL Summarization**: Summarize web content from URLs using Kagi API
- **Multi-language Support**: Automatic translation with language detection

## Project Structure

### Core Modules

- `src/main.rs`: Application entry point, sets up web server routes
- `src/config.rs`: Configuration management
- `src/models.rs`: Shared data structures and error types
- `src/utils.rs`: Utility functions
- `src/rss.rs`: RSS feed parsing functionality

### API Integration

- `src/api/chatgpt.rs`: ChatGPT API integration for AI summaries, translation, and language detection
  - `get_chatgpt_summary()`: Generate comprehensive summaries of multiple stories
  - `get_single_story_summary()`: Generate concise 2-3 sentence summaries for individual stories
  - `translate()`: Translate content to target languages
  - `get_language_code()`: Detect language from user input
- `src/api/line.rs`: LINE Messaging API with Flex Message support
  - `create_stories_carousel()`: Build swipeable carousel cards with stories and summaries
  - `create_summary_bubble()`: Create formatted summary cards with headers and sections
  - `create_text_message()`: Simple text messages
  - `validate_signature()`: HMAC-SHA256 webhook signature verification
- `src/api/kagi.rs`: Kagi API for web page summarization

### Request Handlers

- `src/handlers/webhook.rs`: Processes incoming LINE webhook events
- `src/handlers/stories.rs`: Manages story retrieval and distribution
- `src/handlers/conversation.rs`: Handles conversation functionality

### Configuration Files

- `config.toml`: Main application configuration
- `secrets.toml`: API keys and secrets
- `prompts.toml`: AI prompts for ChatGPT

## API Endpoints

- `POST /webhook`: Receive and process LINE webhook events with signature validation
- `GET /hello`: Health check endpoint
- `GET /getLatestTitle`: Fetch the latest story title (JSON response)
- `GET /getLatestStories`: Fetch all latest stories (JSON response)
- `GET /sendTodayStories`: Broadcast stories as Flex Message carousel with individual AI summaries
- `GET /broadcastDailySummary`: Broadcast comprehensive daily summary as formatted Flex bubble
- `POST /conversation`: Process a conversation directly with the AI

### Message Formats

**Story Carousel** (`/sendTodayStories`):
- Swipeable cards, each showing:
  - Story rank (#1, #2, etc.)
  - Story title
  - 2-3 sentence AI summary
  - "Read Article" button

**Daily Summary** (`/broadcastDailySummary`):
- Single formatted card with:
  - Header with date
  - Comprehensive summary with sections
  - Visual separators between sections

## Dependencies

### Core Runtime & Web
- `tokio`: Asynchronous runtime with full features
- `futures`: Concurrent async operations for parallel API calls
- `warp`: Web framework for handling HTTP requests

### Serialization & Data
- `serde` & `serde_json`: Serialization and deserialization
- `config`: Configuration file management

### HTTP & APIs
- `reqwest`: HTTP client for API calls

### Data Processing
- `rss`: RSS feed parsing
- `scraper`: HTML parsing for story extraction
- `chrono`: Date and time formatting for message headers

### Security
- `base64`, `hmac`, `sha2`: HMAC-SHA256 signature verification for LINE webhooks
- `openssl`: TLS support

### Utilities
- `log`, `env_logger`: Logging infrastructure
- `thiserror`: Error type derivation
- `bytes`: Working with byte sequences
- `url`: URL parsing and validation

## Setup & Configuration

1. Create a `secrets.toml` file with the following keys:
   ```toml
   [channel]
   secret = "YOUR_LINE_CHANNEL_SECRET"
   token = "YOUR_LINE_CHANNEL_TOKEN"
   
   [chatgpt]
   secret = "YOUR_OPENAI_API_KEY"
   
   [kagi]
   secret = "YOUR_KAGI_API_KEY"
   ```

2. Configure endpoints and settings in `config.toml`

3. Configure AI prompts in `prompts.toml`:
   - `summary_all`: Comprehensive summary of all stories
   - `summary_single`: Concise 2-3 sentence summary for individual stories
   - `get_language_code`: Language detection prompt
   - `translate`: Translation prompt template

## UI/UX Features

### LINE Flex Messages
The bot uses LINE's Flex Message format to create rich, interactive cards:

**Carousel Cards for Stories:**
```
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│ #1  Hacker News  │  │ #2  Hacker News  │  │ #3  Hacker News  │
├──────────────────┤  ├──────────────────┤  ├──────────────────┤
│ Story Title      │  │ Story Title      │  │ Story Title      │
│ ───────────────  │  │ ───────────────  │  │ ───────────────  │
│ AI-generated     │  │ AI-generated     │  │ AI-generated     │
│ summary here...  │  │ summary here...  │  │ summary here...  │
│                  │  │                  │  │                  │
│ [Read Article]   │  │ [Read Article]   │  │ [Read Article]   │
└──────────────────┘  └──────────────────┘  └──────────────────┘
    ← Swipe to browse stories →
```

**Summary Bubble:**
- Formatted header with emoji and date
- Sectioned content with visual separators
- Orange accent color (#FF6B35) for branding

### Performance Optimizations
- **Concurrent API Calls**: Story summaries are generated in parallel using `futures::join_all`
- **UTF-8 Safe Truncation**: Character-aware string truncation prevents panics with multi-byte characters
- **Smart Caching**: Configuration loaded once at startup and cached globally
- **Retry Logic**: Automatic retry with exponential backoff for RSS feed fetching

## Running the Application

### Using Docker

Build the Docker image:
```bash
docker build -f docker/Dockerfile -t daily_hacker_news_bot .
```

Run the container:
```bash
docker run -p 3030:3030 daily_hacker_news_bot
```

### Running Locally

```bash
cargo run
```

## Development

Run tests:
```bash
cargo test
```

Run with debug logging:
```bash
RUST_LOG=debug cargo run
```

## License

MIT
