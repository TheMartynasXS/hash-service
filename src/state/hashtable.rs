use directories_next::ProjectDirs;
use hash_service::hash_loader_server::HashLoader;
use hash_service::{
    AddHashRequest, AddHashResponse, GetStringRequest, GetStringResponse, LoadHashesRequest,
    LoadHashesResponse, UnloadHashesRequest, UnloadHashesResponse,
};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tonic::{Request, Response, Status};
use walkdir::WalkDir;
use xxhash_rust::xxh64::xxh64;

pub mod hash_service {
    tonic::include_proto!("hashservice");
}

#[derive(Debug, Clone)]
pub struct ServiceHashLoader {
    game_hashes: Arc<RwLock<HashMap<u64, String>>>,
    bin_hashes: Arc<RwLock<HashMap<u64, String>>>,
    loading_state: Arc<RwLock<LoadingState>>,
}

enum HashtableType {
    Game,
    Bin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadingState {
    Unloaded,
    Loading,
    Loaded,
}

#[tonic::async_trait]
impl HashLoader for ServiceHashLoader {
    async fn load_hashes(
        &self,
        request: Request<LoadHashesRequest>,
    ) -> Result<Response<LoadHashesResponse>, Status> {
        println!("load_hashes called: {:?}", request);

        // Set state to Loading
        {
            let mut state_guard = self
                .loading_state
                .write()
                .map_err(|_| Status::internal("Failed to lock loading state"))?;
            *state_guard = LoadingState::Loading;
        }

        let result = self.load_hashes_impl().await;

        // Update state based on result
        {
            let mut state_guard = self
                .loading_state
                .write()
                .map_err(|_| Status::internal("Failed to lock loading state"))?;
            *state_guard = if result.is_ok() {
                LoadingState::Loaded
            } else {
                LoadingState::Unloaded
            };
        }

        match result {
            Ok(()) => {
                let (game_count, bin_count) = self.get_counts()?;
                Ok(Response::new(LoadHashesResponse {
                    success: true,
                    message: format!(
                        "Hashtables loaded: {} game, {} bin hashes!",
                        game_count, bin_count
                    ),
                    count: (game_count + bin_count) as i32,
                }))
            }
            Err(e) => Ok(Response::new(LoadHashesResponse {
                success: false,
                message: format!("Failed to load hashtables: {}", e),
                count: 0,
            })),
        }
    }

    async fn get_string(
        &self,
        request: Request<GetStringRequest>,
    ) -> Result<Response<GetStringResponse>, Status> {
        let req = request.into_inner();
        println!(
            "get_string called for hash: {}, type: {}",
            req.hash, req.hashtable_type
        );

        self.ensure_loaded_status().await?;

        let hashtable_type = match req.hashtable_type.as_str() {
            "game" => HashtableType::Game,
            "bin" => HashtableType::Bin,
            _ => {
                return Ok(Response::new(GetStringResponse {
                    found: false,
                    value: String::new(),
                }));
            }
        };

        let hashtable = self.get_hashtable(&hashtable_type);
        let guard = hashtable
            .read()
            .map_err(|_| Status::internal("Failed to lock hashtable"))?;

        let response = guard
            .get(&req.hash)
            .map(|value| GetStringResponse {
                found: true,
                value: value.clone(),
            })
            .unwrap_or_else(|| GetStringResponse {
                found: false,
                value: String::new(),
            });

        Ok(Response::new(response))
    }

    async fn unload_hashes(
        &self,
        _request: Request<UnloadHashesRequest>,
    ) -> Result<Response<UnloadHashesResponse>, Status> {
        println!("unload_hashes called");

        // Clear the hashtables to free memory
        {
            let mut game_guard = self
                .game_hashes
                .write()
                .map_err(|_| Status::internal("Failed to lock game hashtable"))?;
            let mut bin_guard = self
                .bin_hashes
                .write()
                .map_err(|_| Status::internal("Failed to lock bin hashtable"))?;

            let game_count = game_guard.len();
            let bin_count = bin_guard.len();

            game_guard.clear();
            bin_guard.clear();

            // Shrink capacity to minimize memory usage
            game_guard.shrink_to_fit();
            bin_guard.shrink_to_fit();

            println!("Unloaded {} game and {} bin hashes", game_count, bin_count);
        }

        // Update state to Unloaded
        {
            let mut state_guard = self
                .loading_state
                .write()
                .map_err(|_| Status::internal("Failed to lock loading state"))?;
            *state_guard = LoadingState::Unloaded;
        }

        Ok(Response::new(UnloadHashesResponse {
            success: true,
            message: "Hashtables unloaded successfully".to_string(),
        }))
    }

    async fn add_hash(
        &self,
        request: Request<AddHashRequest>,
    ) -> Result<Response<AddHashResponse>, Status> {
        let req = request.into_inner();
        println!(
            "add_hash called for , value: {}, type: {}",
            req.string, req.hashtable_type
        );

        self.ensure_loaded_status().await?;

        // if game xxhash64 if bin fnv1a
        let hash = match req.hashtable_type.as_str() {
            "game" => xxh64(req.string.to_lowercase().as_bytes(), 0),
            "bin" => {
                // Correct FNV-1a 32-bit implementation (standard offset basis)
                // 32-bit offset basis: 0x811C9DC5, prime: 0x01000193
                let mut hash: u32 = 0x811C9DC5;
                for &byte in req.string.to_lowercase().as_bytes() {
                    hash ^= byte as u32;
                    hash = hash.wrapping_mul(0x01000193);
                }
                // return as u64 with value in lower 32 bits (matches client 32-bit hex -> decimal)
                hash as u64
            }
            _ => {
                return Ok(Response::new(AddHashResponse {
                    success: false,
                    message: "Invalid hashtable type".to_string(),
                }));
            }
        };
        println!("Computed hash: {}", hash);

        // Insert into appropriate hashtable
        let hashtable = match req.hashtable_type.as_str() {
            "game" => &self.game_hashes,
            "bin" => &self.bin_hashes,
            _ => unreachable!(),
        };
        let mut guard = hashtable
            .write()
            .map_err(|_| Status::internal("Failed to lock hashtable for writing"))?;

        guard.insert(hash, req.string);

        Ok(Response::new(AddHashResponse {
            success: true,
            message: "Added hash successfully".to_string(),
        }))
    }
}

impl ServiceHashLoader {
    pub fn new() -> Self {
        ServiceHashLoader {
            game_hashes: Arc::new(RwLock::new(HashMap::default())),
            bin_hashes: Arc::new(RwLock::new(HashMap::default())),
            loading_state: Arc::new(RwLock::new(LoadingState::Unloaded)),
        }
    }

    async fn ensure_loaded_status(&self) -> Result<(), Status> {
        self.ensure_loaded()
            .await
            .map_err(|e| Status::internal(format!("Failed to load hashtables: {}", e)))
    }

    async fn ensure_loaded(&self) -> Result<(), String> {
        // Check current state and transition if needed
        let should_load = {
            let mut state_guard = self
                .loading_state
                .write()
                .map_err(|_| "Failed to lock loading state".to_string())?;

            match *state_guard {
                LoadingState::Loaded => {
                    // Already loaded, nothing to do
                    return Ok(());
                }
                LoadingState::Loading => {
                    // Already loading, return error
                    return Err("Hashtables are currently being loaded".to_string());
                }
                LoadingState::Unloaded => {
                    // Transition to Loading
                    *state_guard = LoadingState::Loading;
                    true
                }
            }
        };

        if should_load {
            println!("Hashtables are unloaded, loading them now...");

            // Load the hashtables
            let result = self.load_hashes_impl().await;

            // Update state based on result
            let mut state_guard = self
                .loading_state
                .write()
                .map_err(|_| "Failed to lock loading state".to_string())?;

            *state_guard = if result.is_ok() {
                LoadingState::Loaded
            } else {
                LoadingState::Unloaded // Reset to Unloaded on error
            };

            result?;
        }

        Ok(())
    }

    fn get_hashtable(&self, hashtable_type: &HashtableType) -> &Arc<RwLock<HashMap<u64, String>>> {
        match hashtable_type {
            HashtableType::Game => &self.game_hashes,
            HashtableType::Bin => &self.bin_hashes,
        }
    }

    fn get_counts(&self) -> Result<(usize, usize), Status> {
        let game_guard = self
            .game_hashes
            .read()
            .map_err(|_| Status::internal("Failed to lock game hashtable"))?;
        let bin_guard = self
            .bin_hashes
            .read()
            .map_err(|_| Status::internal("Failed to lock bin hashtable"))?;
        Ok((game_guard.len(), bin_guard.len()))
    }

    async fn load_hashes_impl(&self) -> Result<(), String> {
        let project_dirs = ProjectDirs::from("io", "LeagueToolkit", "ltk-hash-cache")
            .ok_or_else(|| "Failed to get project directories".to_string())?;

        let hash_dir: PathBuf = if cfg!(target_os = "linux") {
            project_dirs.cache_dir().to_path_buf()
        } else {
            directories_next::UserDirs::new()
                .and_then(|ud| {
                    ud.document_dir()
                        .map(|p| p.join("LeagueToolkit").join("ltk-hash-cache").to_path_buf())
                })
                .unwrap_or_else(|| project_dirs.cache_dir().to_path_buf())
        };
        std::fs::create_dir_all(&hash_dir)
            .map_err(|e| format!("Failed to create cache directory: {}", e))?;

        // Sync hashtables from GitHub
        let cache_dir_str = hash_dir
            .to_str()
            .ok_or_else(|| "Invalid cache directory path".to_string())?;
        sync_hashtables(cache_dir_str).await?;

        // Load hashtables from directory
        self.add_from_dir(hash_dir)?;

        Ok(())
    }

    fn add_from_dir(&self, dir: impl AsRef<Path>) -> Result<(), String> {
        println!("Loading hashtables from dir: {:?}", dir.as_ref());

        for entry in WalkDir::new(dir).into_iter().filter_map(|x| x.ok()) {
            if !entry.file_type().is_file()
                || entry.path().extension().map_or(false, |ext| ext == "sha")
            {
                continue;
            }

            let file_name = entry.file_name().to_string_lossy();
            let is_game = file_name.contains(".game.");
            let is_bin = file_name.contains(".binentries.");

            if is_game || is_bin {
                println!("Loading hashtable: {:?}", entry.path());
                let mut file = File::open(entry.path())
                    .map_err(|e| format!("Failed to open file {:?}: {}", entry.path(), e))?;
                self.add_from_file(&mut file, is_game)?;
            }
        }

        println!("Hashtables loaded successfully");
        Ok(())
    }

    fn add_from_file(&self, file: &mut File, to_game: bool) -> Result<(), String> {
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut count = 0;

        let hashtable = if to_game {
            &self.game_hashes
        } else {
            &self.bin_hashes
        };

        let mut guard = hashtable
            .write()
            .map_err(|_| "Failed to lock hashtable for writing".to_string())?;

        while let Some(Ok(line)) = lines.next() {
            let mut components = line.split(' ');

            let hash_str = components
                .next()
                .ok_or_else(|| "Failed to read hash from line".to_string())?;
            let hash = u64::from_str_radix(hash_str, 16)
                .map_err(|e| format!("Failed to convert hash '{}': {}", hash_str, e))?;
            let path = components.collect::<Vec<_>>().join(" ");

            guard.insert(hash, path);
            count += 1;
        }

        println!("Loaded {} entries from file", count);
        Ok(())
    }
}

// fn create_project_dirs(project_dirs: &ProjectDirs) {

//     let cache_dir = project_dirs.cache_dir();
//     // check if directory exists
//     if !cache_dir.exists() {
//         // create directory
//         if let Err(e) = std::fs::create_dir_all(cache_dir) {
//             eprintln!("Failed to create cache directory: {:?}", e);
//         }
//     }
// }

async fn sync_hashtables(appdatadir: &str) -> Result<(), String> {
    let git_links: Vec<&str> = vec![
        "https://api.github.com/repos/CommunityDragon/Data/contents/hashes/lol/hashes.binentries.txt",
        "https://api.github.com/repos/CommunityDragon/Data/contents/hashes/lol/hashes.game.txt.0",
        "https://api.github.com/repos/CommunityDragon/Data/contents/hashes/lol/hashes.game.txt.1",
    ];

    for git_url in git_links {
        println!("Syncing hashtable from: {}", git_url);
        let git_data = get_git_data(git_url)
            .await
            .map_err(|e| format!("Failed to fetch data from GitHub: {}", e))?;

        let checksum = git_data
            .get("sha")
            .and_then(|s| s.as_str())
            .ok_or_else(|| "Missing 'sha' field in response".to_string())?;
        let url = git_data
            .get("download_url")
            .and_then(|s| s.as_str())
            .ok_or_else(|| "Missing 'download_url' field in response".to_string())?;
        let file_name = git_data
            .get("name")
            .and_then(|s| s.as_str())
            .ok_or_else(|| "Missing 'name' field in response".to_string())?;

        let file_path = PathBuf::from(appdatadir).join(file_name);

        if file_path.exists() {
            // Append .sha to the file name (e.g., hashes.game.txt.0 -> hashes.game.txt.0.sha)
            let sha_path = file_path.with_file_name(format!("{}.sha", file_name));
            if sha_path.exists() {
                if let Ok(existing_sha) = std::fs::read_to_string(&sha_path) {
                    if existing_sha.trim() == checksum {
                        println!("File {} is up to date, skipping", file_name);
                        continue;
                    }
                }
            }
            println!("File {} needs update, downloading...", file_name);
        } else {
            println!("File {} not found, downloading...", file_name);
        }

        let data = download_file(url)
            .await
            .map_err(|e| format!("Failed to download file: {}", e))?;
        std::fs::write(&file_path, data).map_err(|e| format!("Failed to write file: {}", e))?;

        let sha_path = file_path.with_file_name(format!("{}.sha", file_name));
        std::fs::write(&sha_path, checksum)
            .map_err(|e| format!("Failed to write SHA file: {}", e))?;
        println!("Successfully synced {}", file_name);
    }
    Ok(())
}

async fn get_git_data(url: &str) -> Result<Value, String> {
    let response = http_get(url).await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!(
            "GitHub API request failed with status {}: {}",
            status, text
        ));
    }

    response.json().await.map_err(|e| e.to_string())
}

async fn download_file(url: &str) -> Result<Vec<u8>, String> {
    let response = http_get(url).await?;

    if !response.status().is_success() {
        return Err(format!("Failed to download file: {}", response.status()));
    }

    response
        .bytes()
        .await
        .map_err(|e| e.to_string())
        .map(|b| b.to_vec())
}

async fn http_get(url: &str) -> Result<reqwest::Response, String> {
    reqwest::Client::new()
        .get(url)
        .header("User-Agent", "Rust-Client")
        .send()
        .await
        .map_err(|e| e.to_string())
}
