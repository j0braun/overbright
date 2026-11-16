//! The render worker pool and the frame's job handles (moved from render/mod.rs).

/// The renderer's own worker threads, for recording and encoding command
/// buffers alongside the main thread. They block while idle (rayon's
/// workers spin a while for more work, which cost a frame about 1 ms of
/// CPU across its threads), and take jobs in the order given, so a job
/// that waits for earlier ones (`Submitted`) never waits on a job behind
/// it, whatever the number of threads.
mod jobs {
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::sync::{Arc, Mutex, OnceLock};

    type Job = Box<dyn FnOnce() + Send>;

    static QUEUE: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();

    /// Run `job` on a worker.
    pub fn spawn(job: impl FnOnce() + Send + 'static) {
        let queue = QUEUE.get_or_init(|| {
            let (tx, rx) = channel::<Job>();
            let rx: Arc<Mutex<Receiver<Job>>> = Arc::new(Mutex::new(rx));
            // Up to four (a frame has five jobs at most, the last two
            // short), and one fewer than the cores, the main thread's.
            let threads = std::thread::available_parallelism().map_or(2, |n| n.get().saturating_sub(1)).clamp(1, 4);
            for i in 0..threads {
                let rx = rx.clone();
                std::thread::Builder::new()
                    .name(format!("render {i}"))
                    .spawn(move || {
                        profiling::register_thread!();
                        loop {
                            let job = rx.lock().unwrap().recv();
                            match job {
                                Ok(job) => {
                                    profiling::scope!("render job");
                                    job()
                                }
                                Err(_) => break,
                            }
                        }
                    })
                    .expect("a render thread");
            }
            Mutex::new(tx)
        });
        let _ = queue.lock().unwrap().send(Box::new(job));
    }
}

/// A command encoder being encoded (wgpu's `finish`: validation and the
/// backend's commands, most of a frame's recording cost) on a render
/// thread, so the next passes can be recorded meanwhile. Submitting the
/// command buffers in recording order leaves the same image.
pub struct Finishing(std::sync::mpsc::Receiver<wgpu::CommandBuffer>);

impl Finishing {
    pub fn start(encoder: wgpu::CommandEncoder) -> Finishing {
        Finishing::spawn(move || encoder.finish())
    }

    /// A command buffer already made (it goes to the GPU in its place in
    /// the list, like the others).
    pub fn ready(buffer: wgpu::CommandBuffer) -> Finishing {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(buffer);
        Finishing(rx)
    }

    /// A command buffer made (recorded and encoded) by `make` on a render
    /// thread.
    pub fn spawn(make: impl FnOnce() -> wgpu::CommandBuffer + Send + 'static) -> Finishing {
        let (tx, rx) = std::sync::mpsc::channel();
        jobs::spawn(move || {
            let _ = tx.send(make());
        });
        Finishing(rx)
    }

    /// The command buffer, once encoded.
    pub fn wait(self) -> wgpu::CommandBuffer {
        self.0.recv().expect("encoder finished")
    }
}

/// A value worked out on a render thread (CPU work of the frame's that
/// needs no GPU access, e.g. the crowd's skinning): started as soon as
/// its inputs are final, waited for where the value is used. Jobs run in
/// the order given, so one queued before a frame's encoders never waits on
/// them, nor they on it.
pub struct Pending<T>(std::sync::mpsc::Receiver<T>);

impl<T: Send + 'static> Pending<T> {
    pub fn spawn(make: impl FnOnce() -> T + Send + 'static) -> Pending<T> {
        let (tx, rx) = std::sync::mpsc::channel();
        jobs::spawn(move || {
            let _ = tx.send(make());
        });
        Pending(rx)
    }

    /// The value, once made (None if the job panicked).
    pub fn wait(self) -> Option<T> {
        self.0.recv().ok()
    }
}

/// Command buffers going to the GPU, in order, from a render thread: the
/// last encoder is encoded there too, after those already under way.
/// Measurement aid ( `NFSU2_ONE_SUBMIT=1`): the scene's buffers are
/// kept and go to the GPU with the frame's end in one submission
/// ([`FrameEnd`]): the same buffers in the same order.
pub struct Submitted(std::sync::mpsc::Receiver<Option<(wgpu::Queue, Vec<wgpu::CommandBuffer>)>>);

impl Submitted {
    pub fn start(queue: wgpu::Queue, encoded: Vec<Finishing>, last: wgpu::CommandEncoder) -> Submitted {
        let (tx, rx) = std::sync::mpsc::channel();
        let keep = crate::env_set!("NFSU2_ONE_SUBMIT");
        jobs::spawn(move || {
            let last = last.finish();
            let mut buffers: Vec<wgpu::CommandBuffer> = encoded.into_iter().map(Finishing::wait).collect();
            buffers.push(last);
            if keep {
                let _ = tx.send(Some((queue, buffers)));
            } else {
                queue.submit(buffers);
                let _ = tx.send(None);
            }
        });
        Submitted(rx)
    }

    /// Once submitted (what is submitted after must wait for this).
    pub fn wait(self) {
        if let Ok(Some((queue, buffers))) = self.0.recv() {
            queue.submit(buffers);
        }
    }

    /// Once encoded: the buffers still to submit (none once submitted).
    fn take(self) -> Vec<wgpu::CommandBuffer> {
        self.0.recv().ok().flatten().map(|(_, b)| b).unwrap_or_default()
    }
}

/// The end of a frame going to the GPU from a render thread: once the
/// scene is submitted ([`Submitted`]) and the post passes encoded, the
/// post and UI command buffers are submitted and the swap chain's image
/// presented, while the main thread goes on to the next frame's
/// simulation. The next frame waits for this ([`FrameEnd::wait`]) before
/// it writes any buffer, so every frame's commands still see their own
/// frame's data, as when the main thread submitted them itself.
pub struct FrameEnd(std::sync::mpsc::Receiver<wgpu::SubmissionIndex>);

impl FrameEnd {
    pub fn start(
        queue: wgpu::Queue,
        scene: Option<Submitted>,
        post: Option<Finishing>,
        ui: wgpu::CommandBuffer,
        frame: Option<wgpu::SurfaceTexture>,
    ) -> FrameEnd {
        FrameEnd::start_with(queue, scene, post, vec![ui], frame)
    }

    /// As [`FrameEnd::start`] with the frame's last command buffers in
    /// order (the UI, then HDR Output's display transform).
    pub fn start_with(
        queue: wgpu::Queue,
        scene: Option<Submitted>,
        post: Option<Finishing>,
        last: Vec<wgpu::CommandBuffer>,
        frame: Option<wgpu::SurfaceTexture>,
    ) -> FrameEnd {
        FrameEnd::start_ui(queue, scene, post, None, last, frame)
    }

    /// As [`FrameEnd::start_with`] with the UI's command buffer still being
    /// encoded on a render thread (`ui`, queued before this, so it never
    /// waits on a later job), submitted after the post passes and before
    /// `last`: the same buffers in the same order.
    pub fn start_ui(
        queue: wgpu::Queue,
        scene: Option<Submitted>,
        post: Option<Finishing>,
        ui: Option<Finishing>,
        last: Vec<wgpu::CommandBuffer>,
        frame: Option<wgpu::SurfaceTexture>,
    ) -> FrameEnd {
        let (tx, rx) = std::sync::mpsc::channel();
        jobs::spawn(move || {
            let mut buffers: Vec<wgpu::CommandBuffer> = scene.map(Submitted::take).unwrap_or_default();
            buffers.extend(post.into_iter().chain(ui).map(Finishing::wait));
            buffers.extend(last);
            let index = queue.submit(buffers);
            if let Some(frame) = frame {
                queue.present(frame);
            }
            let _ = tx.send(index);
        });
        FrameEnd(rx)
    }

    /// Once submitted and presented: the frame's last submission.
    pub fn wait(self) -> Option<wgpu::SubmissionIndex> {
        self.0.recv().ok()
    }
}
