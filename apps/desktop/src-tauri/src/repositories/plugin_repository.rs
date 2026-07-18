use crate::dto::PluginWriteDto;
use crate::models::Plugin;

const PLUGIN_STATE_ENABLED: &str = "enabled";
const PLUGIN_STATE_DISABLED: &str = "disabled";

pub async fn get_plugins(db: &mut toasty::Db) -> toasty::Result<Vec<Plugin>> {
    let plugins = Plugin::all().exec(db).await?;
    Ok(plugins)
}

pub async fn get_plugin(db: &mut toasty::Db, id: &str) -> toasty::Result<Plugin> {
    let plugin = Plugin::get_by_id(db, id).await?;
    Ok(plugin)
}

pub async fn add_plugin(db: &mut toasty::Db, plugin: PluginWriteDto) -> toasty::Result<Plugin> {
    let id = plugin.id.clone();

    toasty::create!(Plugin {
        id: id.clone(),
        name: plugin.name,
        version: plugin.version,
        description: plugin.description,
        author: plugin.author,
        link: plugin.link,
        r#type: plugin.r#type,
        permissions: plugin.permissions,
        tags: plugin.tags,
        status: plugin.status,
        category: plugin.category,
        icon: plugin.icon,
        cli_available: plugin.cli_available,
        state: plugin.state,
    })
    .exec(db)
    .await?;

    let plugin = Plugin::get_by_id(db, &id).await?;
    Ok(plugin)
}

pub async fn update_plugin(
    db: &mut toasty::Db,
    mut plugin: Plugin,
    update: PluginWriteDto,
) -> toasty::Result<Plugin> {
    let id = plugin.id.clone();

    toasty::update!(plugin {
        name: update.name,
        version: update.version,
        description: update.description,
        author: update.author,
        link: update.link,
        r#type: update.r#type,
        permissions: update.permissions,
        tags: update.tags,
        status: update.status,
        category: update.category,
        icon: update.icon,
        cli_available: update.cli_available,
        state: update.state,
    })
    .exec(db)
    .await?;

    let plugin = Plugin::get_by_id(db, &id).await?;
    Ok(plugin)
}

pub async fn enable_plugin(db: &mut toasty::Db, id: String) -> toasty::Result<Plugin> {
    update_plugin_state(db, id, PLUGIN_STATE_ENABLED).await
}

pub async fn disable_plugin(db: &mut toasty::Db, id: String) -> toasty::Result<Plugin> {
    update_plugin_state(db, id, PLUGIN_STATE_DISABLED).await
}

pub async fn remove_plugin(db: &mut toasty::Db, id: String) -> toasty::Result<()> {
    Plugin::get_by_id(db, &id).await?;
    Plugin::delete_by_id(db, &id).await?;
    Ok(())
}

async fn update_plugin_state(
    db: &mut toasty::Db,
    id: String,
    state: &str,
) -> toasty::Result<Plugin> {
    let mut plugin = Plugin::get_by_id(db, &id).await?;

    toasty::update!(plugin {
        state: state.to_string(),
    })
    .exec(db)
    .await?;

    let plugin = Plugin::get_by_id(db, &id).await?;
    Ok(plugin)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> toasty::Db {
        let db = toasty::Db::builder()
            .models(toasty::models!(Plugin))
            .connect("sqlite://memory")
            .await
            .expect("in-memory database should connect");
        db.push_schema()
            .await
            .expect("plugin schema should be created");
        db
    }

    fn plugin(id: &str) -> PluginWriteDto {
        PluginWriteDto {
            id: id.to_string(),
            name: format!("Plugin {id}"),
            version: "1.0.0".to_string(),
            description: Some(format!("Description for {id}")),
            author: Some("FlowTools".to_string()),
            link: None,
            r#type: "app".to_string(),
            permissions: vec!["storage".to_string()],
            tags: vec!["test".to_string()],
            status: Some("stable".to_string()),
            category: Some("test".to_string()),
            icon: None,
            cli_available: false,
            state: "registered".to_string(),
        }
    }

    #[tokio::test]
    async fn repository_supports_plugin_lifecycle() {
        let mut db = test_db().await;

        let created = add_plugin(&mut db, plugin("alpha-plugin"))
            .await
            .expect("plugin should be added");
        assert_eq!(created.id, "alpha-plugin");
        assert_eq!(created.state, "registered");

        let fetched = get_plugin(&mut db, "alpha-plugin")
            .await
            .expect("plugin should be fetched");
        assert_eq!(fetched.name, "Plugin alpha-plugin");

        let plugins = get_plugins(&mut db)
            .await
            .expect("plugins should be listed");
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].id, "alpha-plugin");

        let mut update = plugin("alpha-plugin");
        update.name = "Updated Alpha".to_string();
        update.version = "2.0.0".to_string();
        update.permissions = vec!["clipboard".to_string()];
        let updated = update_plugin(&mut db, fetched, update)
            .await
            .expect("plugin should be updated");
        assert_eq!(updated.name, "Updated Alpha");
        assert_eq!(updated.version, "2.0.0");
        assert_eq!(updated.permissions, vec!["clipboard"]);

        let enabled = enable_plugin(&mut db, "alpha-plugin".to_string())
            .await
            .expect("plugin should be enabled");
        assert_eq!(enabled.state, "enabled");

        let disabled = disable_plugin(&mut db, "alpha-plugin".to_string())
            .await
            .expect("plugin should be disabled");
        assert_eq!(disabled.state, "disabled");

        remove_plugin(&mut db, "alpha-plugin".to_string())
            .await
            .expect("plugin should be removed");
        assert!(get_plugin(&mut db, "alpha-plugin").await.is_err());
        assert!(get_plugins(&mut db)
            .await
            .expect("plugins should list")
            .is_empty());
    }

    #[tokio::test]
    async fn repository_rejects_duplicate_and_unknown_plugin_ids() {
        let mut db = test_db().await;

        add_plugin(&mut db, plugin("alpha-plugin"))
            .await
            .expect("first plugin should be added");
        let mut duplicate = plugin("alpha-plugin");
        duplicate.name = "Overwritten".to_string();
        assert!(add_plugin(&mut db, duplicate).await.is_err());
        let original = get_plugin(&mut db, "alpha-plugin")
            .await
            .expect("original plugin should remain");
        assert_eq!(original.name, "Plugin alpha-plugin");

        let unknown = "unknown-plugin".to_string();
        assert!(get_plugin(&mut db, &unknown).await.is_err());
        assert!(enable_plugin(&mut db, unknown.clone()).await.is_err());
        assert!(disable_plugin(&mut db, unknown.clone()).await.is_err());
        assert!(remove_plugin(&mut db, unknown).await.is_err());
    }

    #[tokio::test]
    async fn repository_operations_are_isolated_by_plugin_id() {
        let mut db = test_db().await;

        add_plugin(&mut db, plugin("alpha-plugin"))
            .await
            .expect("alpha should be added");
        add_plugin(&mut db, plugin("beta-plugin"))
            .await
            .expect("beta should be added");

        let alpha = get_plugin(&mut db, "alpha-plugin")
            .await
            .expect("alpha should exist");
        let mut alpha_update = plugin("alpha-plugin");
        alpha_update.name = "Updated Alpha".to_string();
        alpha_update.version = "2.0.0".to_string();
        update_plugin(&mut db, alpha, alpha_update)
            .await
            .expect("alpha should update");
        enable_plugin(&mut db, "alpha-plugin".to_string())
            .await
            .expect("alpha should enable");

        let beta = get_plugin(&mut db, "beta-plugin")
            .await
            .expect("beta should still exist");
        assert_eq!(beta.name, "Plugin beta-plugin");
        assert_eq!(beta.version, "1.0.0");
        assert_eq!(beta.permissions, vec!["storage"]);
        assert_eq!(beta.tags, vec!["test"]);
        assert_eq!(beta.state, "registered");

        remove_plugin(&mut db, "alpha-plugin".to_string())
            .await
            .expect("alpha should be removed");
        let plugins = get_plugins(&mut db)
            .await
            .expect("plugins should be listed");
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].id, "beta-plugin");
    }
}
