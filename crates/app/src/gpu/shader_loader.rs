use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;

pub struct ShaderEntry {
    pub name: String,
    pub source: String,
    pub path: Option<PathBuf>, // None for built-in shaders
}

pub enum ShaderEvent {
    Modified(PathBuf),
    Created(PathBuf),
    Removed(PathBuf),
}

pub struct ShaderLoader {
    shaders: Vec<ShaderEntry>,
    current_index: usize,
    event_rx: mpsc::Receiver<ShaderEvent>,
    _watcher: Option<RecommendedWatcher>,
}

impl ShaderLoader {
    pub fn new(extra_dirs: &[PathBuf]) -> Self {
        let (event_tx, event_rx) = mpsc::channel();

        // Built-in shaders
        let mut shaders = vec![
            ShaderEntry {
                name: "spectrum_rings".to_string(),
                source: include_str!("shaders/spectrum_rings.wgsl").to_string(),
                path: None,
            },
            ShaderEntry {
                name: "audio_wave".to_string(),
                source: include_str!("shaders/audio_wave.wgsl").to_string(),
                path: None,
            },
            ShaderEntry {
                name: "plasma".to_string(),
                source: include_str!("shaders/plasma.wgsl").to_string(),
                path: None,
            },
            ShaderEntry {
                name: "tunnel".to_string(),
                source: include_str!("shaders/tunnel.wgsl").to_string(),
                path: None,
            },
            ShaderEntry {
                name: "fractal".to_string(),
                source: include_str!("shaders/fractal.wgsl").to_string(),
                path: None,
            },
            ShaderEntry {
                name: "kaleidoscope".to_string(),
                source: include_str!("shaders/kaleidoscope.wgsl").to_string(),
                path: None,
            },
        ];

        // Watch shader directories
        let shader_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("terminal-vibes")
            .join("shaders");

        let mut dirs_to_watch = vec![shader_dir];
        dirs_to_watch.extend(extra_dirs.iter().cloned());

        // Load existing .wgsl files from watched dirs
        for dir in &dirs_to_watch {
            if dir.exists() {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().is_some_and(|e| e == "wgsl") {
                            if let Ok(source) = std::fs::read_to_string(&path) {
                                let name = path
                                    .file_stem()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string();
                                shaders.push(ShaderEntry {
                                    name,
                                    source,
                                    path: Some(path),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Set up file watcher
        let tx = event_tx;
        let watcher_result = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    for path in &event.paths {
                        if path.extension().is_some_and(|e| e == "wgsl") {
                            let shader_event = match event.kind {
                                EventKind::Create(_) => Some(ShaderEvent::Created(path.clone())),
                                EventKind::Modify(_) => Some(ShaderEvent::Modified(path.clone())),
                                EventKind::Remove(_) => Some(ShaderEvent::Removed(path.clone())),
                                _ => None,
                            };
                            if let Some(e) = shader_event {
                                let _ = tx.send(e);
                            }
                        }
                    }
                }
            },
            Config::default(),
        );

        let watcher = match watcher_result {
            Ok(mut w) => {
                for dir in &dirs_to_watch {
                    if dir.exists() {
                        let _ = w.watch(dir, RecursiveMode::NonRecursive);
                    }
                }
                Some(w)
            }
            Err(e) => {
                log::warn!("Failed to set up shader watcher: {}", e);
                None
            }
        };

        Self {
            shaders,
            current_index: 0,
            event_rx,
            _watcher: watcher,
        }
    }

    pub fn current(&self) -> Option<&ShaderEntry> {
        self.shaders.get(self.current_index)
    }

    pub fn current_name(&self) -> &str {
        self.current().map(|s| s.name.as_str()).unwrap_or("none")
    }

    pub fn current_source(&self) -> &str {
        self.current().map(|s| s.source.as_str()).unwrap_or("")
    }

    pub fn next(&mut self) {
        if !self.shaders.is_empty() {
            self.current_index = (self.current_index + 1) % self.shaders.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.shaders.is_empty() {
            self.current_index = if self.current_index == 0 {
                self.shaders.len() - 1
            } else {
                self.current_index - 1
            };
        }
    }

    /// Poll for file changes. Returns true if the current shader was modified
    /// (caller should rebuild pipeline).
    pub fn poll_changes(&mut self) -> bool {
        let mut current_changed = false;

        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                ShaderEvent::Modified(path) => {
                    let idx = self
                        .shaders
                        .iter()
                        .position(|s| s.path.as_ref() == Some(&path));
                    if let Some(idx) = idx {
                        match std::fs::read_to_string(&path) {
                            Ok(source) => {
                                self.shaders[idx].source = source;
                                if idx == self.current_index {
                                    current_changed = true;
                                }
                                log::info!("Shader reloaded: {}", self.shaders[idx].name);
                            }
                            Err(e) => {
                                log::warn!("Failed to read shader {}: {}", path.display(), e)
                            }
                        }
                    }
                }
                ShaderEvent::Created(path) => match std::fs::read_to_string(&path) {
                    Ok(source) => {
                        let name = path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        log::info!("New shader discovered: {}", name);
                        self.shaders.push(ShaderEntry {
                            name,
                            source,
                            path: Some(path),
                        });
                    }
                    Err(e) => {
                        log::warn!("Failed to read new shader {}: {}", path.display(), e)
                    }
                },
                ShaderEvent::Removed(path) => {
                    if let Some(idx) = self
                        .shaders
                        .iter()
                        .position(|s| s.path.as_ref() == Some(&path))
                    {
                        let name = self.shaders[idx].name.clone();
                        log::info!("Shader removed: {}", name);
                        self.shaders.remove(idx);
                        if self.shaders.is_empty() {
                            self.current_index = 0;
                        } else if self.current_index >= self.shaders.len() {
                            self.current_index = self.shaders.len() - 1;
                            current_changed = true;
                        } else if idx == self.current_index {
                            current_changed = true;
                        } else if idx < self.current_index {
                            self.current_index -= 1;
                        }
                    }
                }
            }
        }

        current_changed
    }

    pub fn shader_count(&self) -> usize {
        self.shaders.len()
    }
}
