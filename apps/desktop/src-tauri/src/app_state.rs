use tokio::sync::Mutex;

pub struct AppState {
    pub db: Mutex<toasty::Db>,
}