use tauri::{AppHandle, Manager};
use crate::models::Plugin;

pub async fn init_db(app: &AppHandle) -> Result<toasty::Db, Box<dyn std::error::Error>> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;

    let db_path = app_data_dir.join("app.sqlite");
    let db_url = format!("sqlite:{}", db_path.to_string_lossy());

    let db = toasty::Db::builder()
        .models(toasty::models!(Plugin))
        .connect(&db_url)
        .await?;

    db.push_schema().await?;

    Ok(db)
}