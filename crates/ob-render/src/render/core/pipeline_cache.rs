//! The driver's pipeline cache kept on disk (moved from render/mod.rs).

/// A Vulkan pipeline cache kept in the asset cache's directory between
/// runs (`wgpu::util::pipeline_cache_key`'s name: vendor and device):
/// its data is handed to the driver at start (wgpu and the driver check
/// that it is theirs, the driver's version included, and start empty
/// otherwise) and written back when it has grown, through a temporary
/// file and a rename. A cache only spares the driver's compiles; the
/// pipelines are the same. `NFSU2_NO_PIPELINE_CACHE=1` (or
/// `NFSU2_NO_CACHE=1`) turns it off.
///
/// "Grown" is measured against the driver's own data for what it took from
/// the file, not the file's length: the same pipelines can come back a few
/// bytes longer or shorter (Mesa writes its objects in its hash table's
/// order, each aligned to 8 bytes, and that order depends on how the table
/// was filled), so a run that added nothing could look larger than the file
/// and write it again. A file the driver refused counts as nothing.
pub(crate) struct DiskPipelineCache {
    pub(crate) cache: wgpu::PipelineCache,
    pub(crate) path: std::path::PathBuf,
    /// The size last read or written.
    pub(crate) saved: std::sync::atomic::AtomicUsize,
    /// One write at a time, each taking the driver's data when it runs: an
    /// earlier save's smaller data written after a later one's would leave
    /// the later pipelines out of the file.
    writing: std::sync::Mutex<()>,
}

impl DiskPipelineCache {
    pub(crate) fn open(adapter: &wgpu::Adapter, device: &wgpu::Device) -> Option<DiskPipelineCache> {
        if !device.features().contains(wgpu::Features::PIPELINE_CACHE)
            || crate::render::debug_env::nfsu2_no_pipeline_cache()
        {
            return None;
        }
        let path = crate::cache::dir()?.join(wgpu::util::pipeline_cache_key(&adapter.get_info())?);
        let data = std::fs::read(&path).ok();
        // SAFETY: the data is what `get_data` gave for this adapter (or
        // anything else, which `fallback` turns into an empty cache:
        // wgpu checks its own header and the driver its own).
        let cache = unsafe {
            device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                label: Some("pipelines"),
                data: data.as_deref(),
                fallback: true,
            })
        };
        // What the driver holds now, in its own form (see above); nothing
        // without a file.
        let held = if data.is_some() { cache.get_data().map_or(0, |d| d.len()) } else { 0 };
        Some(DiskPipelineCache { cache, path, saved: held.into(), writing: Default::default() })
    }

    /// Write the cache back if it has grown since it was read or written
    /// (on a thread of its own).
    pub(crate) fn save(self: &std::sync::Arc<Self>) {
        let me = self.clone();
        crate::cache::spawn_writer(move || {
            let _one = me.writing.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(data) = me.cache.get_data() else { return };
            if data.len() <= me.saved.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            // Whole or not at all, nothing left behind if killed
            // (`cache::replace_file`).
            if crate::cache::replace_file(&me.path, &data).is_ok() {
                me.saved.store(data.len(), std::sync::atomic::Ordering::Relaxed);
            }
        });
    }
}
