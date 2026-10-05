use crate::models::Plugin;
use tauri::{AppHandle, Manager};

pub async fn init_db(app: &AppHandle) -> Result<toasty::Db, Box<dyn std::error::Error>> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;

    let db_path = app_data_dir.join("app.sqlite");
    init_db_path(&db_path).await
}

async fn init_db_path(db_path: &std::path::Path) -> Result<toasty::Db, Box<dyn std::error::Error>> {
    let should_create_schema = !db_path.exists();
    if !should_create_schema {
        flowtools_runtime_core::data::verify_legacy_database(db_path)
            .map_err(|_| std::io::Error::other("LEGACY_DATABASE_INVALID"))?;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dto::PluginWriteDto, repositories::plugin_repository};

    #[tokio::test]
    async fn reopening_debug_database_preserves_existing_plugin_records() {
        let directory = std::env::temp_dir().join(format!(
            "flowtools-validation-desktop-db-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("app.sqlite");
        let mut db = init_db_path(&path).await.unwrap();
        plugin_repository::add_plugin(
            &mut db,
            PluginWriteDto {
                id: "saved-fixture".into(),
                name: "Preserved".into(),
                version: "0.1.0".into(),
                description: None,
                author: None,
                link: None,
                r#type: "app".into(),
                permissions: vec![],
                tags: vec![],
                status: None,
                category: None,
                icon: None,
                cli_available: false,
                state: "registered".into(),
            },
        )
        .await
        .unwrap();
        drop(db);
        let mut reopened = init_db_path(&path).await.unwrap();
        let record = plugin_repository::get_plugin(&mut reopened, "saved-fixture")
            .await
            .unwrap();
        assert_eq!(record.name, "Preserved");
    }

    #[tokio::test]
    async fn refused_corrupt_database_does_not_delete_the_original_file() {
        let directory = std::env::temp_dir().join(format!(
            "flowtools-validation-corrupt-db-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("app.sqlite");
        std::fs::write(&path, b"preserve-corrupt-source").unwrap();
        assert!(init_db_path(&path).await.is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"preserve-corrupt-source");
    }
}
