use crate::dto::CreatePluginDto;
use crate::models::Plugin;

pub async fn get_plugins(db: &mut toasty::Db) ->toasty::Result<Vec<Plugin>>{
    let plugins = Plugin::all().exec(db).await?;
    Ok(plugins)
}

pub async fn add_plugin(db: &mut toasty::Db, plugin: CreatePluginDto) -> toasty::Result<()> {
    toasty::create!(Plugin {
        id: plugin.id,
        name: plugin.name,
        version: plugin.version,
        description: plugin.description,
        r#type: plugin.r#type,
        permissions: plugin.permissions,
    }).exec(db).await?;
    Ok(())
}