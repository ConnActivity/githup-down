use crate::Thread;
use serde_json::Value;
use sqlx::{Pool, Postgres};

pub async fn get_stackoverflow_post_list() -> Result<Vec<Thread>, reqwest::Error> {
    let response = reqwest::Client::new().get("https://api.stackexchange.com/2.3/questions?page=11&pagesize=100&order=desc&sort=votes&site=stackoverflow")
        .header("Accept", "application/json")
        .send()
        .await?
        .text().await?;
    println!("{}", response);
    let vals: Value = serde_json::from_str(&response).unwrap();
    let mut posts: Vec<Thread> = Vec::new();
    for val in vals["items"].as_array().unwrap() {
        let post = Thread {
            id: val["question_id"].as_i64().unwrap(),
            title: val["title"].as_str().unwrap().to_string(),
            comments: val["answer_count"].as_i64().unwrap(),
            score: val["score"].as_i64().unwrap(),
        };
        posts.push(post);
    }
    Ok(posts)
}

async fn insert_threads(pool: &Pool<Postgres>, threads: Vec<Thread>) -> Result<(), sqlx::Error> {
    // Insert the repo into the `repos` table using batch insert
    let mut tx = pool.begin().await?;
    for thread in threads {
        sqlx::query!(
            "INSERT INTO threads (id, title, comments, score) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
            thread.id as i32,
            thread.title,
            thread.comments as i32,
            thread.score as i32
        )
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tracing::info;

    #[tokio::test]
    async fn pull_stackoverflow_posts() {
        tracing_subscriber::fmt::init();
        let posts = get_stackoverflow_post_list().await.unwrap();
        assert!(posts.len() > 0);
        let database_url = env::var("DATABASE_URL").unwrap();
        let pool = sqlx::postgres::PgPool::connect(&database_url)
            .await
            .unwrap();
        insert_threads(&pool, posts).await.unwrap();
        info!("Inserted posts into database");
    }
}
