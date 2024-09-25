PRAGMA foreign_keys = 0;

DROP TABLE conversations;

CREATE TABLE conversations (
    message        INTEGER  PRIMARY KEY
                            UNIQUE
                            NOT NULL,
    channel        INTEGER  NOT NULL,
    guild          INTEGER  NOT NULL,
    parent         INTEGER  REFERENCES conversations (message) ON DELETE SET NULL,
    input          TEXT     NOT NULL,
    output         TEXT     NOT NULL,
    time           DATETIME NOT NULL
                            DEFAULT (datetime() ),
    system_message TEXT
)
WITHOUT ROWID;

PRAGMA foreign_keys = 1;