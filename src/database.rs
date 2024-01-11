use std::path::Path;

use sqlx::{
	migrate,
	sqlite::{SqliteConnectOptions, SqlitePoolOptions},
	Pool, Sqlite,
};

pub async fn init_database<P: AsRef<Path>>(path: P) -> Pool<Sqlite> {
	let pool = SqlitePoolOptions::new()
		.max_connections(4)
		.connect_with(
			SqliteConnectOptions::new()
				.filename(path)
				.create_if_missing(true),
		)
		.await
		.unwrap();

	migrate!("./migrations")
		.run(&pool)
		.await
		.expect("Failed to apply migrations");

	pool
}
