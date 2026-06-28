use crate::models::Plugin;
use tauri::{AppHandle, Manager};

pub async fn init_db(app: &AppHandle) -> Result<toasty::Db, Box<dyn std::error::Error>> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;

    let db_path = app_data_dir.join("app.sqlite");

    #[cfg(debug_assertions)]
    {
        const RESET_DB_ON_START: bool = true;

        if RESET_DB_ON_START && db_path.exists() {
            std::fs::remove_file(&db_path)?;
        }
    }

    let should_create_schema = !db_path.exists();
    let db_url = format!("sqlite:{}", db_path.to_string_lossy());

    let db = toasty::Db::builder()
        .models(toasty::models!(Plugin))
        .connect(&db_url)
        .await?;

    if should_create_schema {
        db.push_schema().await?;
    }

    Ok(db)
}
