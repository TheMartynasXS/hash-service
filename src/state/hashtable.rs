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

pub mod hash_service {
    tonic::include_proto!("hashservice");
}

#[derive(Debug, Clone, Default)]
pub struct ServiceHashLoader {
    game_hashes: Arc<RwLock<HashMap<u64, String>>>,
    bin_hashes: Arc<RwLock<HashMap<u64, String>>>,
}

#[tonic::async_trait]
impl HashLoader for ServiceHashLoader {
    async fn load_hashes(
        &self,
        request: Request<LoadHashesRequest>,
    ) -> Result<Response<LoadHashesResponse>, Status> {
        println!("load_hashes called: {:?}", request);

        // Sync and load hashtables
        match self.load_hashes_impl().await {
            Ok(()) => {
                // Get the total count of loaded hashes
                let game_guard = self
                    .game_hashes
                    .read()
                    .map_err(|_| Status::internal("Failed to lock game hashtable"))?;
                let bin_guard = self
                    .bin_hashes
                    .read()
                    .map_err(|_| Status::internal("Failed to lock bin hashtable"))?;

                let total_count = (game_guard.len() + bin_guard.len()) as i32;

                let response = LoadHashesResponse {
                    success: true,
                    message: format!(
                        "Hashtables loaded: {} game, {} bin hashes!",
                        game_guard.len(),
                        bin_guard.len()
                    ),
                    count: total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                let response = LoadHashesResponse {
                    success: false,
                    message: format!("Failed to load hashtables: {}", e),
                    count: 0,
                };
                Ok(Response::new(response))
            }
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

        let hashtable = match req.hashtable_type.as_str() {
            "game" => &self.game_hashes,
            "bin" => &self.bin_hashes,
            _ => {
                return Ok(Response::new(GetStringResponse {
                    found: false,
                    value: String::new(),
                }));
            }
        };

        let hashtable_guard = hashtable
            .read()
            .map_err(|_| Status::internal("Failed to lock hashtable"))?;

        let response = match hashtable_guard.get(&req.hash) {
            Some(value) => GetStringResponse {
                found: true,
                value: value.clone(),
            },
            None => GetStringResponse {
                found: false,
                value: String::new(),
            },
        };

        Ok(Response::new(response))
    }

    async fn unload_hashes(
        &self,
        _request: Request<UnloadHashesRequest>,
    ) -> Result<Response<UnloadHashesResponse>, Status> {
        println!("unload_hashes called");

        // Placeholder implementation
        let response = UnloadHashesResponse {
            success: true,
            message: "unload_hashes - placeholder".to_string(),
        };
        Ok(Response::new(response))
    }

    async fn add_hash(
        &self,
        request: Request<AddHashRequest>,
    ) -> Result<Response<AddHashResponse>, Status> {
        let req = request.into_inner();
        println!(
            "add_hash called for hash: {}, value: {}, type: {}",
            req.hash, req.value, req.hashtable_type
        );

        // Placeholder implementation
        let response = AddHashResponse {
            success: true,
            message: "add_hash - placeholder".to_string(),
        };
        Ok(Response::new(response))
    }
}

impl ServiceHashLoader {
    pub fn new() -> Self {
        ServiceHashLoader {
            game_hashes: Arc::new(RwLock::new(HashMap::default())),
            bin_hashes: Arc::new(RwLock::new(HashMap::default())),
        }
    }

    async fn load_hashes_impl(&self) -> Result<(), String> {
        let project_dirs = ProjectDirs::from("com", "league-toolkit", "lol-hashes");

        let project_dirs =
            project_dirs.ok_or_else(|| "Failed to get project directories".to_string())?;
        let cache_dir = project_dirs.cache_dir();
        create_project_dirs(&project_dirs);

        // Sync hashtables from GitHub
        let cache_dir_str = cache_dir
            .to_str()
            .ok_or_else(|| "Invalid cache directory path".to_string())?;
        sync_hashtables(cache_dir_str).await?;

        // Load hashtables from directory
        self.add_from_dir(cache_dir)?;

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
            .map_err(|_| "Failed to lock hashtable".to_string())?;

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

fn create_project_dirs(project_dirs: &ProjectDirs) {
    let cache_dir = project_dirs.cache_dir();
    // check if directory exists
    if !cache_dir.exists() {
        // create directory
        if let Err(e) = std::fs::create_dir_all(cache_dir) {
            eprintln!("Failed to create cache directory: {:?}", e);
        }
    }
}

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
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("User-Agent", "Rust-Client")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!(
            "GitHub API request failed with status {}: {}",
            status, text
        ));
    }

    let v: Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(v)
}

async fn download_file(url: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("User-Agent", "Rust-Client")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Failed to download file: {}", response.status()));
    }

    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    Ok(bytes.to_vec())
}
