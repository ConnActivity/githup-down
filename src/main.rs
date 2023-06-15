mod github;
mod routing;
mod stackoverflow;

use crate::github::get_image_for_repo;
use crate::routing::{BoxFut, Route, SmallRouter};
use async_once_cell::OnceCell;
use dotenvy::dotenv;
use lambda_http::{run, service_fn, Body, Error, Request, RequestExt, Response};
use liquid::{object, Template};
use reqwest::{get, header};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Pool, Postgres};
use std::env;
use std::fmt::Debug;
use tokio::task;

use tracing::info;

const INDEX: &str = include_str!("web/index.liquid");
const GITHUB_GAME: &str = include_str!("web/github.liquid");
const STACKOVERFLOW_GAME: &str = include_str!("web/stackoverflow.liquid");
static INSTANCE: OnceCell<PgPool> = OnceCell::new();
fn get_random_repository(req: Request) -> BoxFut {
    Box::pin(async move {
        let repo = get_random_repository_from_db(INSTANCE.get().unwrap()).await;
        Ok(Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!(repo).to_string()))
            .unwrap())
    })
}
/// This is the main body for the function.
/// Write your code inside it.
/// There are some code example in the following URLs:
/// - https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/examples

async fn get_image_from_url(url: String) -> String {
    let own_url = url.split("/");
    let mut own_url = own_url.collect::<Vec<&str>>();
    let name = own_url.pop().unwrap().to_string();
    let owner = own_url.pop().unwrap().to_string();
    tokio::spawn(async move {
        let image = get_image_for_repo(owner, name).await;
        image
    })
    .await
    .unwrap()
}
const DATABASE_URL: &str = env!("DATABASE_URL");
fn insert_into_db(req: Request) -> BoxFut {
    Box::pin(async move {
        let pool = INSTANCE.get().unwrap();
        let request_split = req.uri().path().split("/");
        let leaderboard = request_split.last().unwrap();
        let body = req.body();
        let new_score: Results = serde_json::from_slice(body.as_ref()).unwrap();
        insert_new_score(&pool, new_score.clone(), leaderboard.to_string()).await;
        Ok(Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!(new_score).to_string()))
            .unwrap())
    })
}

fn get_random_repos(req: Request) -> BoxFut {
    Box::pin(async move {
        let pool = INSTANCE.get().unwrap();
        let count = req
            .query_string_parameters()
            .iter()
            .find(|(k, _)| k == &"count")
            .unwrap_or(("count", "1"))
            .1
            .parse::<u8>()?;
        let repo_futures = (0..count)
            .into_iter()
            .map(|_| get_random_repository_from_db(&pool));
        let repos = futures::future::join_all(repo_futures).await;
        Ok(Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!(repos).to_string()))
            .unwrap())
    })
}
fn get_random_thread(req: Request) -> BoxFut {
    Box::pin(async move {
        let pool = INSTANCE.get().unwrap();
        let count = req
            .query_string_parameters()
            .iter()
            .find(|(k, _)| k == &"count")
            .unwrap_or(("count", "1"))
            .1
            .parse::<u8>()?;
        let thread_futures = (0..count)
            .into_iter()
            .map(|_| get_random_thread_from_db(&pool));
        let threads = futures::future::join_all(thread_futures).await;
        Ok(Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!(threads).to_string()))
            .unwrap())
    })
}

async fn function_handler(event: Request) -> Result<Response<Body>, Error> {
    // Initialize the database pool
    dotenv().ok();
    let pool = INSTANCE
        .get_or_try_init({
            println!("Creating pool");
            PgPoolOptions::new()
                .max_connections(5)
                .connect(DATABASE_URL)
        })
        .await?;
    let routes = vec![
        Route::new(
            lambda_http::http::Method::GET,
            "/random-github-repo".to_string(),
            get_random_repository,
        ),
        Route::new(lambda_http::http::Method::GET, "/".to_string(), index_view),
        Route::new(
            lambda_http::http::Method::GET,
            "/github-star-game".to_string(),
            github_star_game,
        ),
        Route::new(
            lambda_http::http::Method::POST,
            "/insert_to_leaderboard/github".to_string(),
            insert_into_db,
        ),
        Route::new(
            lambda_http::http::Method::GET,
            "/random-github-repos".to_string(),
            get_random_repos,
        ),
        Route::new(
            lambda_http::http::Method::POST,
            "/insert_to_leaderboard/stackoverflow".to_string(),
            insert_into_db,
        ),
        Route::new(
            lambda_http::http::Method::GET,
            "/stackoverflow-upvote-game".to_string(),
            stackoverflow_upvote_game,
        ),
        Route::new(
            lambda_http::http::Method::GET,
            "/random-stackoverflow-thread".to_string(),
            get_random_thread,
        ),
    ];
    let router = SmallRouter { routes };
    router.route(event).await.map_err(|e| {
        println!("Error: {:?}", e);
        lambda_http::Error::from(format!("Error: {:?}", e))
    })
}
fn github_star_game(req: Request) -> BoxFut {
    Box::pin(async move {
        let pool = INSTANCE.get().unwrap();
        // Tasks to avoid blocking on every request
        async fn get_random_repo_with_image(pool: &PgPool) -> (Repositories, String) {
            let repo = get_random_repository_from_db(pool).await;
            let image = get_image_from_url(repo.clone().uri).await;
            (repo, image)
        }
        let left_handle = task::spawn(async move { get_random_repo_with_image(&pool).await });
        let right_handle = task::spawn(async move { get_random_repo_with_image(&pool).await });
        let (left, image_left) = left_handle.await.unwrap();
        let (right, image_right) = right_handle.await.unwrap();
        let template = liquid_parse(GITHUB_GAME.to_string());
        let mut globals = object!({
            "left": left,
            "right": right,
            "image_left": image_left,
            "image_right": image_right,
        });
        let body = template.render(&mut globals).unwrap();
        Ok(Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "text/html")
            .body(Body::from(body))
            .unwrap())
    })
}
fn stackoverflow_upvote_game(req: Request) -> BoxFut {
    Box::pin(async move {
        let pool = INSTANCE.get().unwrap();
        // Tasks to avoid blocking on every request
        let left_handle = task::spawn(async move { get_random_thread_from_db(pool).await });
        let right_handle = task::spawn(async move { get_random_thread_from_db(pool).await });

        let template = liquid_parse(STACKOVERFLOW_GAME.to_string());
        let left = left_handle.await.unwrap();
        let right = right_handle.await.unwrap();
        let mut globals = object!({
            "left": left,
            "right": right,
        });
        let body = template.render(&mut globals).unwrap();
        Ok(Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "text/html")
            .body(Body::from(body))
            .unwrap())
    })
}

fn index_view(req: Request) -> BoxFut {
    Box::pin(async move {
        let pool = INSTANCE
            .get_or_try_init({
                println!("Creating pool");
                PgPoolOptions::new()
                    .max_connections(5)
                    .connect(DATABASE_URL)
            })
            .await?;
        let github_handle =
            task::spawn(async move { get_leaderboard(&pool, "github".to_string()).await.results });
        let stackoverflow_handle = task::spawn(async move {
            get_leaderboard(&pool, "stackoverflow".to_string())
                .await
                .results
        });
        let github_leaderboard = github_handle.await.unwrap();
        let stackoverflow_leaderboard = stackoverflow_handle.await.unwrap();
        let template = liquid_parse(INDEX.to_string());
        let mut globals = object!({
            "github_leaderboard": json!(github_leaderboard).to_string(),
            "stackoverflow_leaderboard": json!(stackoverflow_leaderboard).to_string() ,
        });
        let body = template.render(&mut globals).unwrap();
        Ok(Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "text/html")
            .body(Body::from(body))
            .unwrap())
    })
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        // disable printing the name of the module in every log line.
        .with_target(false)
        // disabling time is handy because CloudWatch will add the ingestion time.
        .without_time()
        .init();
    run(service_fn(function_handler)).await
}

pub(crate) fn liquid_parse(text: String) -> Template {
    let compiler = liquid::ParserBuilder::with_stdlib()
        .build()
        .expect("Could not build liquid compiler");
    compiler.parse(text.as_str()).unwrap()
}
async fn get_github_repository_list(url: &str) -> Vec<Repositories> {
    let response = reqwest::Client::new()
        .get(url)
        .header(header::USER_AGENT, "Meris-Agent-P")
        .header(header::ACCEPT, "application/json")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", env!("GITHUB_TOKEN")),
        )
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .unwrap();
    let response_text = response.text().await.unwrap();
    let root: Value = serde_json::from_str(response_text.as_str()).unwrap();
    // FIXME: This crashes sometimes
    let items = root["items"].as_array().unwrap();
    let mut repositories: Vec<Repositories> = Vec::new();
    for item in items {
        let repository = Repositories {
            id: item["id"].as_i64().unwrap(),
            name: item["name"].as_str().unwrap().to_string(),
            description: item["description"].as_str().unwrap_or("").to_string(),
            uri: item["html_url"].as_str().unwrap().to_string(),
            stargazers_count: item["stargazers_count"].as_i64().unwrap(),
            image_url: Option::from(
                get_image_from_url(item["html_url"].as_str().unwrap().to_string()).await,
            ),
        };
        repositories.push(repository);
    }
    repositories
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Repositories {
    id: i64,
    name: String,
    description: String,
    uri: String,
    stargazers_count: i64,
    image_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thread {
    id: i64,
    title: String,
    comments: i64,
    score: i64,
}

async fn insert_repos(pool: &Pool<Postgres>, repos: Vec<Repositories>) -> Result<(), sqlx::Error> {
    // Insert the repo into the `repos` table using batch insert
    let mut tx = pool.begin().await?;
    for repo in repos {
        sqlx::query!(
            "INSERT INTO repos (id, name, description, uri, stargazers_count, image_url) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING",
            repo.id as i32,
            repo.name,
            repo.description,
            repo.uri,
            repo.stargazers_count as i32,
            repo.image_url
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
async fn get_repository_from_db(pool: &Pool<Postgres>, index: i64) -> Repositories {
    // Get the first row from the `repos` table
    let repo = sqlx::query!("SELECT * FROM repos LIMIT 1 OFFSET $1", index - 1)
        .fetch_one(pool)
        .await
        .unwrap();
    Repositories {
        id: repo.id as i64,
        name: repo.name,
        description: repo.description.unwrap(),
        uri: repo.uri.clone(),
        stargazers_count: repo.stargazers_count as i64,
        image_url: Option::from(get_image_from_url(repo.uri).await),
    }
}
async fn get_random_thread_from_db(pool: &Pool<Postgres>) -> Thread {
    let thread = sqlx::query!("SELECT * FROM threads ORDER BY RANDOM() LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap();
    Thread {
        id: thread.id as i64,
        title: thread.title,
        comments: thread.comments as i64,
        score: thread.score as i64,
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Results {
    name: String,
    streak: i32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Leaderboard {
    game_mode: String,
    results: Vec<Results>,
}

async fn insert_new_score(pool: &Pool<Postgres>, result: Results, game_mode: String) {
    let existing_score: Option<(i32,)> =
        sqlx::query_as("SELECT streak FROM leaderboard WHERE name = $1 AND game_mode = $2")
            .bind(&result.name)
            .bind(&game_mode)
            .fetch_optional(pool)
            .await
            .unwrap();

    if let Some((existing_streak,)) = existing_score {
        if result.streak <= existing_streak {
            info!(
                "Score for {} not updated because existing streak is higher",
                result.name
            );
            return;
        }
    }
    sqlx::query(
        "INSERT INTO leaderboard (name, streak, game_mode)
         VALUES ($1, $2, $3)
         ON CONFLICT (name, game_mode)
         DO UPDATE SET streak = EXCLUDED.streak",
    )
    .bind(&result.name)
    .bind(result.streak)
    .bind(game_mode)
    .execute(pool)
    .await
    .unwrap();
    info!("Inserted or updated score for {}", result.name);
}

async fn get_leaderboard(pool: &Pool<Postgres>, game_mode: String) -> Leaderboard {
    let results: Vec<Results> = sqlx::query!(
        r"
        SELECT name, streak
        FROM leaderboard
        WHERE game_mode = $1
        ORDER BY streak DESC
        LIMIT 10
        ",
        game_mode
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|row| Results {
        name: row.name,
        streak: row.streak,
    })
    .collect();
    let leaderboard = Leaderboard { game_mode, results };
    info!("Leaderboard: {:?}", leaderboard);
    leaderboard
}
async fn get_random_repository_from_db(pool: &Pool<Postgres>) -> Repositories {
    // Get a random row from the `repos` table
    let repo = sqlx::query!("SELECT * FROM repos ORDER BY RANDOM() LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap();
    info!("Repo: {:?}", repo);
    Repositories {
        id: repo.id as i64,
        name: repo.name,
        description: repo.description.unwrap(),
        uri: repo.uri.clone(),
        stargazers_count: repo.stargazers_count as i64,
        image_url: Option::from(get_image_from_url(repo.uri).await),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::get;
    #[tokio::test]
    async fn insert_into_db() {
        tracing_subscriber::fmt::init();
        info!("Scraping repositories...");
        let database_url = env::var("DATABASE_URL").unwrap();
        let pool = INSTANCE
            .get_or_try_init({
                println!("Creating pool");
                PgPoolOptions::new()
                    .max_connections(5)
                    .connect(database_url.as_str())
            })
            .await
            .unwrap();
        for i in 1..=10 {
            info!("Scraping page {}", i);
            let repos = get_github_repository_list(
                format!(
                    "https://api.github.com/search/repositories?q=stars:>1000&per_page=100&sort=created&direction=asc&page={}",
                    i
                )
                .as_str(),
            )
            .await;
            insert_repos(&pool, repos).await.unwrap();
        }
    }
}
