--
-- File generated with SQLiteStudio v3.2.1 on Wed Jan 3 14:58:42 2024
--
-- Text encoding used: System
--

-- Table: allowances
CREATE TABLE allowances (
    user         INTEGER  PRIMARY KEY ON CONFLICT REPLACE
                          UNIQUE
                          NOT NULL,
    time_to_full DATETIME NOT NULL
)
WITHOUT ROWID;


-- Table: conversations
CREATE TABLE conversations (
    message        INTEGER  PRIMARY KEY
                            UNIQUE
                            NOT NULL,
    parent         INTEGER  REFERENCES conversations (message) ON DELETE SET NULL,
    input          TEXT     NOT NULL,
    output         TEXT     NOT NULL,
    time           DATETIME NOT NULL
                            DEFAULT (datetime() ),
    system_message TEXT
)
WITHOUT ROWID;


-- Table: spending
CREATE TABLE spending (
    user          INTEGER  NOT NULL,
    cost          INTEGER  NOT NULL,
    input_tokens  INTEGER  NOT NULL,
    output_tokens INTEGER  NOT NULL,
    model         TEXT     NOT NULL,
    time          DATETIME DEFAULT (datetime() ) 
                           NOT NULL
);


-- Table: user_settings
CREATE TABLE user_settings (
    user           INTEGER PRIMARY KEY
                           UNIQUE
                           NOT NULL,
    temperature    REAL,
    max_tokens     INTEGER,
    model          TEXT,
    system_message TEXT
);

