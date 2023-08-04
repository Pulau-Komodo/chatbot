--
-- File generated with SQLiteStudio v3.4.4 on Fri Aug 4 18:10:38 2023
--
-- Text encoding used: System
--
PRAGMA foreign_keys = off;
BEGIN TRANSACTION;

-- Table: allowances
CREATE TABLE IF NOT EXISTS allowances (user INTEGER PRIMARY KEY ON CONFLICT REPLACE UNIQUE NOT NULL, time_to_full DATETIME NOT NULL) WITHOUT ROWID;

-- Table: conversations
CREATE TABLE IF NOT EXISTS conversations (message INTEGER PRIMARY KEY UNIQUE NOT NULL, parent INTEGER REFERENCES conversations (message) ON DELETE SET NULL, input TEXT NOT NULL, output TEXT NOT NULL, time DATETIME NOT NULL DEFAULT (datetime()), system_message TEXT) WITHOUT ROWID;

-- Table: spending
CREATE TABLE IF NOT EXISTS spending (user INTEGER NOT NULL, cost INTEGER NOT NULL, input_tokens INTEGER NOT NULL, output_tokens INTEGER NOT NULL, model TEXT NOT NULL, time DATETIME DEFAULT (datetime()) NOT NULL);

-- Table: user_settings
CREATE TABLE IF NOT EXISTS user_settings (user INTEGER PRIMARY KEY UNIQUE NOT NULL, temperature REAL, max_tokens INTEGER, model TEXT, system_message TEXT);

COMMIT TRANSACTION;
PRAGMA foreign_keys = on;
