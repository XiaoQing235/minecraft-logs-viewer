use crate::{
    cache::Cache,
    document::{Document, Output},
    save::{self, TargetStamp},
    search::{self, Hit, Options, Report},
    source::{self, LogSource},
    text::{self, IndexedLog},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub enum Event {
    Source(u64, LogSource),
    Scanned(u64, Vec<String>),
    Loaded(u64, Arc<IndexedLog>),
    Failed(u64, String),
    Hits(u64, Vec<Hit>),
    Searched(u64, Report),
    Saved(u64, usize, PathBuf, TargetStamp, String, String, Output),
}

pub struct SaveRequest {
    pub editor: usize,
    pub document: Document,
    pub text: String,
    pub output: Output,
    pub target: PathBuf,
    pub expected: Option<TargetStamp>,
}

struct Task<C> {
    id: u64,
    cancel: Arc<AtomicBool>,
    command: C,
}
struct Pending<C> {
    task: Option<Task<C>>,
    stop: bool,
}
struct Lane<C> {
    pending: Arc<(Mutex<Pending<C>>, Condvar)>,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl<C: Send + 'static> Lane<C> {
    fn new(mut execute: impl FnMut(Task<C>) + Send + 'static) -> Self {
        let pending = Arc::new((
            Mutex::new(Pending::<C> {
                task: None,
                stop: false,
            }),
            Condvar::new(),
        ));
        let shared = pending.clone();
        let handle = thread::spawn(move || {
            loop {
                let (lock, ready) = &*shared;
                let mut state = lock.lock().unwrap();
                while state.task.is_none() && !state.stop {
                    state = ready.wait(state).unwrap();
                }
                if state.stop {
                    break;
                }
                let task = state.task.take().unwrap();
                drop(state);
                execute(task);
            }
        });
        Self {
            pending,
            cancel: Arc::new(AtomicBool::new(false)),
            handle: Some(handle),
        }
    }
    fn replace(&mut self, id: u64, command: C) {
        self.cancel();
        self.cancel = Arc::new(AtomicBool::new(false));
        let (lock, ready) = &*self.pending;
        lock.lock().unwrap().task = Some(Task {
            id,
            cancel: self.cancel.clone(),
            command,
        });
        ready.notify_one();
    }
    fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl<C> Drop for Lane<C> {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        let (lock, ready) = &*self.pending;
        lock.lock().unwrap().stop = true;
        ready.notify_one();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn send(sender: &SyncSender<Event>, mut event: Event, cancel: &AtomicBool) {
    loop {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        match sender.try_send(event) {
            Ok(()) => {
                fltk::app::awake();
                return;
            }
            Err(TrySendError::Disconnected(_)) => return,
            Err(TrySendError::Full(returned)) => {
                event = returned;
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

pub struct Workers {
    scan: Lane<(PathBuf, bool)>,
    read: Lane<(LogSource, Option<String>)>,
    search: Lane<(Vec<LogSource>, Options)>,
    save: Lane<SaveRequest>,
    pub events: Receiver<Event>,
    pub cache: Cache,
    next: u64,
}
impl Workers {
    pub fn new() -> std::io::Result<Self> {
        let (sender, events) = mpsc::sync_channel(64);
        let cache = Cache::new()?;
        let scan_sender = sender.clone();
        let scan = Lane::new(move |task: Task<(PathBuf, bool)>| {
            let mut failures = Vec::new();
            source::scan(
                &task.command.0,
                task.command.1,
                &task.cancel,
                |source| send(&scan_sender, Event::Source(task.id, source), &task.cancel),
                |failure| failures.push(failure),
            );
            send(
                &scan_sender,
                Event::Scanned(task.id, failures),
                &task.cancel,
            );
        });
        let read_cache = cache.clone();
        let read_sender = sender.clone();
        let read = Lane::new(move |task: Task<(LogSource, Option<String>)>| {
            let result = text::open(
                task.command.0,
                task.command.1.as_deref(),
                &read_cache,
                &task.cancel,
                |log| send(&read_sender, Event::Loaded(task.id, log), &task.cancel),
            );
            if let Err(error) = result {
                send(
                    &read_sender,
                    Event::Failed(task.id, error.to_string()),
                    &task.cancel,
                );
            }
        });
        let search_cache = cache.clone();
        let search_sender = sender.clone();
        let search = Lane::new(move |task: Task<(Vec<LogSource>, Options)>| {
            match search::run(
                task.command.0,
                task.command.1,
                &search_cache,
                &task.cancel,
                |hits| send(&search_sender, Event::Hits(task.id, hits), &task.cancel),
            ) {
                Ok(report) => send(
                    &search_sender,
                    Event::Searched(task.id, report),
                    &task.cancel,
                ),
                Err(error) => send(
                    &search_sender,
                    Event::Failed(task.id, error.to_string()),
                    &task.cancel,
                ),
            }
        });
        let save = Lane::new(move |task: Task<SaveRequest>| {
            let request = task.command;
            let result = (|| -> std::io::Result<_> {
                let bytes = request.document.encode(&request.text, &request.output)?;
                let stamp = save::write_copy(
                    &request.target,
                    &bytes,
                    &request.document.protected,
                    &request.expected,
                    &task.cancel,
                )?;
                let (original, _, _) = request.output.encoding.decode(&bytes);
                Ok((stamp, original.into_owned()))
            })();
            match result {
                Ok((stamp, original)) => send(
                    &sender,
                    Event::Saved(
                        task.id,
                        request.editor,
                        request.target,
                        stamp,
                        request.text,
                        original,
                        request.output,
                    ),
                    &task.cancel,
                ),
                Err(error) => send(
                    &sender,
                    Event::Failed(task.id, error.to_string()),
                    &task.cancel,
                ),
            }
        });
        Ok(Self {
            scan,
            read,
            search,
            save,
            events,
            cache,
            next: 0,
        })
    }
    fn next(&mut self) -> u64 {
        self.next += 1;
        self.next
    }
    pub fn scan(&mut self, root: PathBuf, recursive: bool) -> u64 {
        let id = self.next();
        self.scan.replace(id, (root, recursive));
        id
    }
    pub fn open(&mut self, source: LogSource, encoding: Option<String>) -> u64 {
        let id = self.next();
        self.read.replace(id, (source, encoding));
        id
    }
    pub fn cancel(&self) {
        self.scan.cancel();
        self.read.cancel();
        self.search.cancel();
    }
    pub fn search(&mut self, sources: Vec<LogSource>, options: Options) -> u64 {
        let id = self.next();
        self.search.replace(id, (sources, options));
        id
    }
    pub fn save(&mut self, request: SaveRequest) -> u64 {
        let id = self.next();
        self.save.replace(id, request);
        id
    }
}
