//! Leptos server functions (type-safe RPC). Bodies run only under `ssr`;
//! on the client these become network calls to `/api/*`.

use leptos::prelude::*;

use crate::types::{
    AddRequest, CalendarEntry, Category, Defaults, DirListing, FileEntry, GrabHistoryEntry, Indexer,
    Library, MediaSearchResult, ProviderInfo, QualityProfile, Release, RssFeed, Settings,
    TorrentDetail, TorrentView, WantedItem,
};

#[server]
pub async fn add_torrent(req: AddRequest) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    // Non-blocking: validates the target dir now, resolves metadata in the
    // background, and surfaces progress/errors via the live stats stream.
    state
        .engine
        .spawn_add_url(req.source, req.output_dir, req.paused, req.only_files)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

/// List a magnet/URL torrent's files without adding it, for the file picker.
#[server]
pub async fn probe_url(source: String, output_dir: String) -> Result<Vec<FileEntry>, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .engine
        .probe_url(source, output_dir)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn dismiss_pending(id: u64) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state.engine.dismiss_pending(id);
    Ok(())
}

#[server]
pub async fn pause_torrent(id: u64) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .engine
        .pause(id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
pub async fn resume_torrent(id: u64) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .engine
        .resume(id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
pub async fn cancel_torrent(id: u64) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .engine
        .cancel(id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
pub async fn delete_torrent(id: u64) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .engine
        .delete(id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
pub async fn browse_dir(path: Option<String>) -> Result<DirListing, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .engine
        .browse(path)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn make_dir(parent: String, name: String) -> Result<DirListing, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .engine
        .make_dir(parent, name)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn get_defaults() -> Result<Defaults, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    Ok(Defaults {
        download_dir: state.config.download_dir.to_string_lossy().into_owned(),
        browse_root: state.config.browse_root.to_string_lossy().into_owned(),
        auth_enabled: state.config.auth_enabled(),
    })
}

#[server]
pub async fn list_torrents() -> Result<Vec<TorrentView>, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    Ok(state.engine.current().torrents.clone())
}

#[server]
pub async fn get_torrent_detail(id: u64) -> Result<TorrentDetail, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .engine
        .detail(id)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Change which files a running torrent downloads.
#[server]
pub async fn update_torrent_files(id: u64, files: Vec<usize>) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .engine
        .update_files(id, files)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn list_categories() -> Result<Vec<Category>, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .list_categories()
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn upsert_category(category: Category) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .upsert_category(category)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn delete_category(slug: String) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .delete_category(&slug)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn list_quality_profiles() -> Result<Vec<QualityProfile>, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .list_quality_profiles()
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn upsert_quality_profile(profile: QualityProfile) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .upsert_quality_profile(profile)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn delete_quality_profile(id: String) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .delete_quality_profile(&id)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn get_provider_info() -> Result<ProviderInfo, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    Ok(state.pvr.provider_info())
}

#[server]
pub async fn set_tmdb_key(key: String) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .set_tmdb_key(key)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn test_tmdb() -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .test_tmdb()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn tmdb_search(query: String) -> Result<Vec<MediaSearchResult>, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .tmdb_search(&query)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

// --- indexers ---------------------------------------------------------------

#[server]
pub async fn list_indexers() -> Result<Vec<Indexer>, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .list_indexers()
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn upsert_indexer(indexer: Indexer) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .upsert_indexer(indexer)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn delete_indexer(id: String) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .delete_indexer(&id)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn test_indexer(indexer: Indexer) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .test_indexer(&indexer)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn search_releases(query: String) -> Result<Vec<Release>, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .search_releases(&query)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

// --- rss feeds --------------------------------------------------------------

#[server]
pub async fn list_feeds() -> Result<Vec<RssFeed>, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .list_feeds()
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn upsert_feed(feed: RssFeed) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .upsert_feed(feed)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn delete_feed(id: String) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .delete_feed(&id)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn poll_feeds_now() -> Result<usize, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .poll_feeds()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn get_feed_poll_mins() -> Result<u32, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    Ok(state.pvr.feed_poll_mins() as u32)
}

#[server]
pub async fn set_feed_poll_mins(mins: u32) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .set_feed_poll_mins(mins)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

// --- wanted / monitor -------------------------------------------------------

#[server]
pub async fn list_wanted() -> Result<Vec<WantedItem>, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .list_wanted()
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn add_wanted(item: WantedItem) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .add_wanted(item)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn remove_wanted(id: String) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .remove_wanted(&id)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn get_calendar() -> Result<Vec<CalendarEntry>, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .get_calendar()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn run_monitor_now() -> Result<usize, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .run_monitor()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

// --- library ----------------------------------------------------------------

#[server]
pub async fn get_library() -> Result<Library, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    Ok(state.pvr.library())
}

#[server]
pub async fn rescan_library() -> Result<Library, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    let pvr = state.pvr.clone();
    tokio::task::spawn_blocking(move || pvr.scan_library())
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Import finished TV downloads into Show/Season folders, then rescan.
#[server]
pub async fn import_now() -> Result<Library, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    let pvr = state.pvr.clone();
    tokio::task::spawn_blocking(move || {
        pvr.import_finished();
        pvr.scan_library()
    })
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))
}

// --- grabbing / history -----------------------------------------------------

#[server]
pub async fn grab_release(
    url: String,
    title: String,
    category: String,
) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .grab_release(&url, &url, &title, &category, "search", 0)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn list_grab_history() -> Result<Vec<GrabHistoryEntry>, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .pvr
        .list_grab_history()
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn get_settings() -> Result<Settings, ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    Ok(state.engine.get_settings())
}

#[server]
pub async fn set_settings(settings: Settings) -> Result<(), ServerFnError> {
    use crate::server::AppState;
    let state = expect_context::<AppState>();
    state
        .engine
        .set_settings(settings)
        .map_err(|e| ServerFnError::new(e.to_string()))
}
