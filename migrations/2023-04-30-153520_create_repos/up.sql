-- Your SQL goes here
CREATE TABLE repos (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    uri TEXT NOT NULL,
    stargazers_count INTEGER NOT NULL,
    image_url TEXT
);

CREATE TABLE leaderboard2
(
    name      TEXT NOT NULL,
    streak    INTEGER NOT NULL,
    game_mode TEXT DEFAULT 'github'::TEXT NOT NULL,
    CONSTRAINT pk_leaderboard2 PRIMARY KEY (name, game_mode),
    CONSTRAINT unique_name_game_mode UNIQUE (name, game_mode)
);

CREATE TABLE threads (
    id SERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    commentscount INTEGER NOT NULL,
    score INTEGER NOT NULL
);