use crate::models::Plugin;

pub async fn get_plugins(db: &mut toasty::Db) ->toasty::Result< Vec<Plugin>>{
    let plugins = Plugin::all().exec(db).await?;
    Ok(plugins)
}