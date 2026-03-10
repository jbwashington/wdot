pub mod mouse;
pub mod profile;
pub mod recorder;
pub mod replay;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use profile::BehaviorProfile;
use recorder::BehaviorRecorder;
use replay::BehaviorReplayer;

/// Manages behavior profiles: recording, storage, and replay.
pub struct BehaviorEngine {
    data_dir: PathBuf,
    active_profile: Arc<RwLock<Option<BehaviorProfile>>>,
    recorder: Arc<RwLock<Option<BehaviorRecorder>>>,
}

impl BehaviorEngine {
    pub fn new(data_dir: PathBuf) -> Self {
        let profiles_dir = data_dir.join("profiles");
        std::fs::create_dir_all(&profiles_dir).ok();

        Self {
            data_dir,
            active_profile: Arc::new(RwLock::new(Some(BehaviorProfile::default_human()))),
            recorder: Arc::new(RwLock::new(None)),
        }
    }

    /// Get a replayer for the active profile.
    pub async fn replayer(&self) -> Option<BehaviorReplayer> {
        let profile = self.active_profile.read().await;
        profile.as_ref().map(|p| BehaviorReplayer::new(p.clone()))
    }

    /// Start recording a new behavior profile.
    pub async fn start_recording(&self, name: String) {
        let mut rec = self.recorder.write().await;
        let mut recorder = BehaviorRecorder::new(name);
        recorder.start();
        *rec = Some(recorder);
    }

    /// Stop recording and save the profile.
    pub async fn stop_recording(&self) -> Option<BehaviorProfile> {
        let mut rec = self.recorder.write().await;
        if let Some(recorder) = rec.take() {
            let profile = recorder.compile_profile();
            self.save_profile(&profile).ok();
            Some(profile)
        } else {
            None
        }
    }

    /// Record a behavior event (while recording is active).
    pub async fn record_event(&self, event: recorder::BehaviorEvent) {
        let mut rec = self.recorder.write().await;
        if let Some(ref mut recorder) = *rec {
            recorder.record_event(event);
        }
    }

    /// Activate a saved profile by name.
    pub async fn activate_profile(&self, name: &str) -> Result<(), String> {
        let profile = self.load_profile(name)?;
        let mut active = self.active_profile.write().await;
        *active = Some(profile);
        Ok(())
    }

    /// List saved profile names.
    pub fn list_profiles(&self) -> Vec<String> {
        let dir = self.data_dir.join("profiles");
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        if name.ends_with(".bin") {
                            Some(name.trim_end_matches(".bin").to_string())
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Delete a saved profile.
    pub fn delete_profile(&self, name: &str) -> Result<(), String> {
        let path = self.data_dir.join("profiles").join(format!("{}.bin", name));
        std::fs::remove_file(path).map_err(|e| format!("Failed to delete profile: {}", e))
    }

    /// Save a profile to disk using bincode.
    fn save_profile(&self, profile: &BehaviorProfile) -> Result<(), String> {
        let path = self
            .data_dir
            .join("profiles")
            .join(format!("{}.bin", profile.name));
        let data =
            bincode::serialize(profile).map_err(|e| format!("Serialization error: {}", e))?;
        std::fs::write(path, data).map_err(|e| format!("Write error: {}", e))
    }

    /// Load a profile from disk.
    fn load_profile(&self, name: &str) -> Result<BehaviorProfile, String> {
        let path = self.data_dir.join("profiles").join(format!("{}.bin", name));
        let data = std::fs::read(path).map_err(|e| format!("Read error: {}", e))?;
        bincode::deserialize(&data).map_err(|e| format!("Deserialization error: {}", e))
    }
}
