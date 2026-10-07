use crate::models::Plugin;
use tauri::{AppHandle, Manager};

pub async fn init_db(app: &AppHandle) -> Result<toasty::Db, Box<dyn std::error::Error>> {
    // Windows Known Folder APIs ignore APPDATA overrides. Validation needs its
    // own explicit directory, guarded by the compiled fixture identity/mode.
    let configured = validation_directory(
        cfg!(debug_assertions),
        &app.config().identifier,
        std::env::var("FLOWTOOLS_RUNTIME_VALIDATION").as_deref() == Ok("1"),
        std::env::var_os("FLOWTOOLS_DESKTOP_VALIDATION_DATA_ROOT").map(std::path::PathBuf::from),
    )?;
    let app_data_dir = match configured {
        Some(directory) => directory,
        None => app.path().app_data_dir()?,
    };
    std::fs::create_dir_all(&app_data_dir)?;

    let db_path = app_data_dir.join("app.sqlite");
    init_db_path(&db_path).await
}

fn validation_directory(
    debug: bool,
    identifier: &str,
    requested: bool,
    path: Option<std::path::PathBuf>,
) -> Result<Option<std::path::PathBuf>, std::io::Error> {
    if let Some(ref path) = path {
        if !debug
            || !requested
            || identifier != crate::RUNTIME_VALIDATION_IDENTIFIER
            || !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("flowtools-validation-"))
            || flowtools_runtime_core::data::safe_path(path).is_err()
        {
            return Err(std::io::Error::other("VALIDATION_PROFILE_REQUIRED"));
        }
    }
    Ok(path)
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

    #[test]
    fn explicit_fixture_directory_requires_debug_identity_mode_and_safe_path() {
        let path = std::env::temp_dir().join("flowtools-validation-desktop");
        for debug in [false, true] {
            for requested in [false, true] {
                for identifier in [
                    crate::RUNTIME_VALIDATION_IDENTIFIER,
                    "com.flowtools.desktop",
                ] {
                    assert_eq!(
                        validation_directory(debug, identifier, requested, Some(path.clone()))
                            .is_ok(),
                        debug && requested && identifier == crate::RUNTIME_VALIDATION_IDENTIFIER
                    );
                }
            }
        }
        assert!(validation_directory(
            true,
            crate::RUNTIME_VALIDATION_IDENTIFIER,
            true,
            Some("flowtools-validation-relative".into())
        )
        .is_err());
    }

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
