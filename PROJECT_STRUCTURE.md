# Project Structure

This repository is a Rust chat application with a WebSocket server, a terminal client, a single-file web UI, and SQL Server persistence.

## Runtime Entry Points

- `src/main.rs`: Chooses the runtime mode. `cargo run` starts the WebSocket server; `cargo run -- client` starts the terminal client.
- `src/server/mod.rs`: Server bootstrap and route registration.
- `src/server/websocket.rs`: Main WebSocket session loop, authentication gate, chat actions, profile/friends/groups/settings actions, and admin user-management actions.
- `src/client/client.rs`: Terminal chat client that logs in, joins rooms, sends public messages, and sends private messages.
- `index.html`: Browser chat UI. It handles login/register, public rooms, private chats, media messages, profile/friends/groups/settings panels, and the admin user-management panel.

## Core Rust Modules

- `src/auth.rs`: Account registration, login, password hashing/verification, session creation, permission loading, and password change logic.
- `src/db.rs`: SQL Server connection setup and all persistence helpers for chat history, profiles, friends, groups, and admin user management.
- `src/chat/message.rs`: Shared `ChatMessage` and `MediaInfo` data models exchanged as JSON between clients and the server.
- `src/chat/message_store.rs`: In-memory message cache used by the server while also persisting chat history to SQL Server.
- `src/chat/mod.rs`, `src/client/mod.rs`, `src/server/mod.rs`: Module exports.

## Database And Scripts

- `SQL ChatDb.txt`: SQL Server schema and migration-style setup for `ChatDB`. It creates users, roles, permissions, friends, groups, group invites/join requests, chat history, and indexes.
- The application expects SQL Server connection settings from environment variables when present:
  - `DB_HOST`
  - `DB_PORT`
  - `DB_NAME`
  - `DB_USER`
  - `DB_PASSWORD`
- If environment variables are not set, `src/db.rs` uses local development defaults.

## Documentation Files

- `README.md`: User-facing setup and usage guide for running the server, terminal client, and web UI.
- `API_SUMMARY.md`: Notes about the JSON message shape and API behavior.
- `MEDIA_FEATURES.md`: Media-message examples and behavior details.
- `CHANGELOG.md`: Feature/change history.
- `CLAUDE.md`: Local coding guidance for AI assistants working in this project.
- `PROJECT_STRUCTURE.md`: This file. It exists so Antigravity and other agents can quickly understand what each major file and folder does.

## Main Data Flow

1. A client connects to `/ws` and sends `login` or `register`.
2. `src/server/websocket.rs` validates the account through `src/auth.rs`.
3. After login, the session carries `user_id`, `username`, `role`, and permissions.
4. Chat, profile, friend, group, settings, and admin actions are routed by `msg_type`.
5. Database-backed operations go through `src/db.rs`.
6. Chat messages are saved to `ChatHistory`, cached in memory, and broadcast to room members or private participants.

## Admin User Management

- Admin actions require the `admin.manage_users` permission.
- `admin_users_list` returns up to 100 users, optionally filtered by username or display name.
- `admin_user_update` changes a user's role between `user` and `admin`, and toggles `is_active`.
- Inactive users cannot log in because `src/auth.rs` rejects accounts where `Users.is_active = 0`.

## Message Time Display

- New messages receive a UNIX timestamp from `ChatMessage::new`.
- History loaded from SQL Server maps `ChatHistory.created_at` into `ChatMessage.timestamp`.
- The web UI displays same-day messages as `HH:mm`; older messages as `dd/MM/yyyy HH:mm`.
