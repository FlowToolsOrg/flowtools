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
