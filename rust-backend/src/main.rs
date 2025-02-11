use anyhow::{bail, Context, Result};
use anyhow_http::{
    http_error_bail,
    response::{HttpErrorResponse, HttpJsonResult, HttpResult},
    ResultExt,
};
use axum::{
    extract::{DefaultBodyLimit, Multipart, State},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use axum_server::Server;
use c2pa::{Builder, CallbackSigner};
use c2pa_crypto::raw_signature::SigningAlg;
use futures::{io::AllowStdIo, AsyncWrite, TryStreamExt};
use hex;
use http_body_util::StreamBody;
use hyper::StatusCode;
use mime_guess;
use multer::parse_boundary;
use once_cell::sync::Lazy;
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    io::{self, Cursor, Seek, Write},
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tempfile::NamedTempFile;
use tokio::fs as tokio_fs;
use tokio::io::AsyncWriteExt;
use tokio_util::compat::FuturesAsyncWriteCompatExt;
use tokio_util::io::{ReaderStream, StreamReader};
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

const PYTHON_BACKEND_URL: &str = "http://python-backend:8001/process_video";
const PYTHON_ANALYZE_URL: &str = "http://python-backend:8001/analyze_video";
const PRIVATE_KEY: &[u8] = include_bytes!("../certs/ed25519.pem");
const PUBLIC_KEY: &[u8] = include_bytes!("../certs/ed25519.pub");

#[derive(Debug, Clone)]
struct AppState {
    processing_status: Arc<Mutex<HashMap<String, ProcessingStatus>>>,
}

#[derive(Debug, Clone, Serialize)]
struct ProcessingStatus {
    status: String,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PythonAnalyzeResponse {
    extracted_bits: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct VideoData {
    path: String,
    message_bits: Vec<f32>,
}

static VIDEO_STORE: Lazy<Mutex<HashMap<String, VideoData>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn message_bits_to_hex(message_bits: &[f32]) -> String {
    // Convert f32 bits to actual bits (0 or 1)
    let bits: Vec<u8> = message_bits
        .iter()
        .map(|&x| if x > 0.5 { 1 } else { 0 })
        .collect();

    // Pack bits into bytes
    let mut bytes = Vec::new();
    for chunk in bits.chunks(8) {
        let mut byte = 0u8;
        for (i, &bit) in chunk.iter().enumerate() {
            if bit == 1 {
                byte |= 1 << (7 - i); // MSB first
            }
        }
        bytes.push(byte);
    }

    // If the number of bits is not a multiple of 8, pad with zeros
    if bits.len() % 8 != 0 {
        bytes.push(0);
    }

    // Convert bytes to hex string
    hex::encode(bytes)
}

fn message_bits_from_hex(hex: &str) -> Vec<f32> {
    let mut bits = Vec::new();
    for chunk in hex.as_bytes().chunks(8) {
        if let Ok(bytes) = std::str::from_utf8(chunk) {
            if let Ok(decoded) = hex::decode(bytes) {
                if let Ok(arr) = decoded.try_into() {
                    bits.push(f32::from_be_bytes(arr));
                }
            }
        }
    }
    bits
}

fn manifest_def(title: &str, format: &str, message_bits: &[f32]) -> String {
    json!({
        "title": title,
        "format": format,
        "claim_generator_info": [{
            "name": "C2PA Watermarking Service",
            "version": "1.0"
        }],
        "assertions": [{
            "label": "c2pa.watermark",
            "data": {
                "message_bits_hex": message_bits_to_hex(message_bits),
                "message_bits_len": message_bits.len(),  // Store original length to handle padding
                "action": "c2pa.watermarked",
                "softwareAgent": {
                    "name": "C2PA Watermarking Service",
                    "version": "1.0"
                }
            }
        }]
    })
    .to_string()
}

async fn create_c2pa_manifest(
    mut video: NamedTempFile,
    message_bits: Vec<f32>,
) -> Result<NamedTempFile> {
    // Create manifest definition
    let format = "video/mp4";
    let title = "watermarked_video.mp4";
    let json = manifest_def(title, format, &message_bits);

    // Create builder from JSON
    let mut builder = Builder::from_json(&json)?;

    // Write the manifest builder to a zipped stream
    let mut zipped = Cursor::new(Vec::new());
    builder.to_archive(&mut zipped)?;

    // Rewind the zipped stream
    zipped.rewind()?;

    // Create signer with our certificates
    let ed_signer =
        |_context: *const (), data: &[u8]| CallbackSigner::ed25519_sign(data, PRIVATE_KEY);
    let signer = CallbackSigner::new(ed_signer, SigningAlg::Ed25519, PUBLIC_KEY);

    // Create builder from archive and sign
    let mut builder = Builder::from_archive(&mut zipped)?;
    let mut dest = NamedTempFile::new_in("./data/temp")?;
    builder.sign(&signer, format, video.as_file_mut(), dest.as_file_mut())?;
    dest.flush()?;

    Ok(dest)
}

async fn process_video(
    input_path: PathBuf,
    output_path: PathBuf,
    status: Arc<Mutex<HashMap<String, ProcessingStatus>>>,
    session_id: String,
) -> Result<()> {
    let client = reqwest::Client::new();

    // Create multipart form for the video
    let form = reqwest::multipart::Form::new().part(
        "video",
        reqwest::multipart::Part::file(&input_path)
            .await
            .unwrap()
            .file_name("video.mp4")
            .mime_str("video/mp4")?,
    );

    // Send to Python backend
    let response = client
        .post(PYTHON_BACKEND_URL)
        .multipart(form)
        .send()
        .await?;

    if response.status().is_success() {
        let boundary = response
            .headers()
            .get("Content-Type")
            .ok_or(multer::Error::IncompleteHeaders)
            .and_then(|header| {
                header
                    .to_str()
                    .map_err(|_| multer::Error::IncompleteHeaders)
            })
            .and_then(|header| parse_boundary(header))
            .context("python response did not contain boundary")?;
        // let boundary
        let mut multipart = multer::Multipart::new(response.bytes_stream(), boundary);

        // Create temporary file for the watermarked video
        let mut temp_file = NamedTempFile::new_in("./data/temp")?;

        let mut file_written = false;
        let mut message_bits_received = false;
        let mut message_bits: Vec<f32> = vec![];
        while let Some(mut field) = multipart.next_field().await? {
            if let Some(name) = field.name() {
                if name == "video" {
                    let body_with_io_error =
                        field.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
                    let body_reader = StreamReader::new(body_with_io_error);
                    futures::pin_mut!(body_reader);
                    tokio::io::copy(
                        &mut body_reader,
                        &mut AllowStdIo::new(temp_file.as_file_mut()).compat_write(),
                    )
                    .await?;
                    file_written = true;
                } else if name == "message_bits" {
                    message_bits = field.json().await?;
                    message_bits_received = true;
                }
            }
        }
        if !file_written {
            bail!("python response did not contain watermarked video");
        }
        if !message_bits_received {
            bail!("python response did not contain message bits");
        }

        // Create C2PA manifest for the video
        let manifest_file = create_c2pa_manifest(temp_file, message_bits.clone())
            .await
            .context("failed to create C2PA manifest")?;

        // Read the signed video and save to output
        manifest_file.persist(&output_path)?;

        // Store the video data in our global store
        {
            let mut store = VIDEO_STORE.lock().unwrap();
            store.insert(
                message_bits_to_hex(&message_bits),
                VideoData {
                    path: output_path.to_string_lossy().to_string(),
                    message_bits,
                },
            );
        }

        // Save store after modification
        if let Err(e) = save_video_store().await {
            eprintln!("Failed to save video store: {}", e);
        }

        // Update status
        status.lock().unwrap().insert(
            session_id,
            ProcessingStatus {
                status: "completed".to_string(),
                error: None,
            },
        );
    } else {
        let error_msg = format!("Failed to process video: {}", response.status());
        status.lock().unwrap().insert(
            session_id,
            ProcessingStatus {
                status: "failed".to_string(),
                error: Some(error_msg.clone()),
            },
        );
        bail!(error_msg)
    }

    Ok(())
}

#[axum::debug_handler]
async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> HttpJsonResult<String> {
    let session_id = Uuid::new_v4().to_string();

    let mut input_file = NamedTempFile::new_in("./data/temp").unwrap();
    let output_path = PathBuf::from("./data/processed").join(format!("{}.mp4", session_id));

    // Initialize status
    state.processing_status.lock().unwrap().insert(
        session_id.clone(),
        ProcessingStatus {
            status: "uploading".to_string(),
            error: None,
        },
    );

    while let Some(mut field) = multipart.next_field().await.unwrap() {
        if field.name().unwrap() == "video" {
            loop {
                let chunk_result = match field.chunk().await {
                    Ok(maybe_chunk) => maybe_chunk,
                    Err(e) => {
                        eprintln!("Error getting chunk: {}", e);
                        http_error_bail!(INTERNAL_SERVER_ERROR, "Failed to process upload: {}", e)
                    }
                };

                // If no more chunks, break the loop
                if chunk_result.is_none() {
                    break;
                }

                // Unwrap is safe here because we checked is_none() above
                let chunk = chunk_result.unwrap();

                if let Err(e) = input_file.write_all(&chunk) {
                    eprintln!("Error writing chunk to file: {}", e);
                    http_error_bail!(INTERNAL_SERVER_ERROR, "failed to write file: {}", e)
                }
            }

            // Update status and start processing
            state.processing_status.lock().unwrap().insert(
                session_id.clone(),
                ProcessingStatus {
                    status: "processing".to_string(),
                    error: None,
                },
            );

            // Process the video in the background
            let state_clone = state.clone();
            let session_id_clone = session_id.clone();
            let status_clone = state.processing_status.clone();
            let session_id_for_error = session_id.clone();
            tokio::spawn(async move {
                if let Err(e) = process_video(
                    input_file.path().to_path_buf(),
                    output_path,
                    state_clone.processing_status,
                    session_id_clone,
                )
                .await
                {
                    eprintln!("Error processing video: {}", e);
                    // Update the processing status with the error
                    let mut status_map = status_clone.lock().unwrap();
                    if let Some(status) = status_map.get_mut(&session_id_for_error) {
                        status.status = "failed".to_string();
                        status.error = Some(e.to_string());
                    }
                }
            });

            return Ok(session_id);
        }
    }
    http_error_bail!(BAD_REQUEST, "No video file found");
}

async fn get_status(
    State(state): State<AppState>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(status) = state.processing_status.lock().unwrap().get(&session_id) {
        axum::Json(status.clone())
    } else {
        axum::Json(ProcessingStatus {
            status: "not_found".to_string(),
            error: Some("Session not found".to_string()),
        })
    }
}

async fn download_file(
    State(state): State<AppState>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let status = state
        .processing_status
        .lock()
        .unwrap()
        .get(&session_id)
        .cloned();

    match status {
        Some(status) if status.status == "completed" => {
            let file_path = PathBuf::from("data/processed").join(format!("{}.mp4", session_id));
            if let Ok(file) = tokio_fs::read(&file_path).await {
                Response::builder()
                    .header("content-type", "video/mp4")
                    .body(axum::body::Body::from(file))
                    .unwrap()
                    .into_response()
            } else {
                Response::builder()
                    .status(404)
                    .body(axum::body::Body::from("File not found"))
                    .unwrap()
                    .into_response()
            }
        }
        Some(status) if status.status == "failed" => Response::builder()
            .status(500)
            .body(axum::body::Body::from(
                status.error.unwrap_or_else(|| "Unknown error".to_string()),
            ))
            .unwrap()
            .into_response(),
        _ => Response::builder()
            .status(400)
            .body(axum::body::Body::from(
                "Video is still processing or not found",
            ))
            .unwrap()
            .into_response(),
    }
}

// File operations for VIDEO_STORE
async fn save_video_store() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let json = {
        let store = VIDEO_STORE.lock().unwrap();
        serde_json::to_string(&*store)?
    };
    tokio_fs::write("data/video_store.json", json).await?;
    Ok(())
}

async fn load_video_store() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match tokio_fs::read("data/video_store.json").await {
        Ok(contents) => {
            let json = String::from_utf8(contents)?;
            let loaded_store: HashMap<String, VideoData> = serde_json::from_str(&json)?;
            let mut store = VIDEO_STORE.lock().unwrap();
            *store = loaded_store;
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()), // File doesn't exist yet
        Err(e) => Err(Box::new(e)),
    }
}

#[derive(Debug, Deserialize)]
struct MessageBitsRequest {
    message_bits_hex: String,
}

fn find_closest_video(
    store: &HashMap<String, VideoData>,
    target_bits: &[f32],
) -> Option<VideoData> {
    store
        .values()
        .map(|video| {
            let distance = video
                .message_bits
                .iter()
                .zip(target_bits.iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f32>();
            (video, distance)
        })
        .min_by(|(_, dist1), (_, dist2)| {
            dist1
                .partial_cmp(dist2)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(video, _)| video.clone())
}

#[axum::debug_handler]
async fn analyze_file(
    _state: State<AppState>,
    mut multipart: Multipart,
) -> HttpJsonResult<impl IntoResponse> {
    let mut temp_file = None;
    while let Some(field) = multipart.next_field().await.unwrap() {
        if field.name().unwrap() == "video" {
            // Create temporary file for the video
            let mut file = NamedTempFile::new_in("./data/temp").unwrap();
            let body_with_io_error = field.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
            let body_reader = StreamReader::new(body_with_io_error);
            futures::pin_mut!(body_reader);
            tokio::io::copy(
                &mut body_reader,
                &mut AllowStdIo::new(file.as_file_mut()).compat_write(),
            )
            .await
            .unwrap();
            temp_file = Some(file);
        }
    }

    if let Some(file) = temp_file {
        let client = reqwest::Client::new();

        let part = match reqwest::multipart::Part::file(file.path()).await {
            Ok(p) => p,
            Err(e) => {
                http_error_bail!(INTERNAL_SERVER_ERROR, "Failed to create file: {}", e);
            }
        };

        // Create multipart form for the video
        let form = reqwest::multipart::Form::new().part(
            "video",
            part.file_name("video.mp4").mime_str("video/mp4").unwrap(),
        );

        // Send to Python backend for analysis
        let response = match client.post(PYTHON_ANALYZE_URL).multipart(form).send().await {
            Ok(resp) => resp,
            Err(e) => {
                http_error_bail!(
                    INTERNAL_SERVER_ERROR,
                    "Failed to connect to Python backend: {}",
                    e
                );
            }
        };

        if response.status().is_success() {
            let python_response: PythonAnalyzeResponse = match response.json().await {
                Ok(resp) => resp,
                Err(e) => {
                    http_error_bail!(
                        INTERNAL_SERVER_ERROR,
                        "Failed to parse response from Python backend: {}",
                        e
                    );
                }
            };

            // Find the closest video in our store
            let video_data = {
                let store = VIDEO_STORE.lock().unwrap();
                find_closest_video(&store, &python_response.extracted_bits)
            };

            if let Some(video_data) = video_data {
                // Clone video data before await
                let video_data = video_data.clone();

                // Open the video file
                match tokio::fs::File::open(&video_data.path).await {
                    Ok(file) => {
                        let stream = ReaderStream::new(file);
                        let body = axum::body::Body::from_stream(stream);

                        let ret = Response::builder()
                            .header(
                                "content-type",
                                mime_guess::from_path(&video_data.path)
                                    .first_or_octet_stream()
                                    .as_ref(),
                            )
                            .header(
                                "content-disposition",
                                format!(
                                    "attachment; filename=\"{}\"",
                                    video_data.path.split('/').last().unwrap_or("video.mp4")
                                ),
                            )
                            .body(body)
                            .map_status(StatusCode::INTERNAL_SERVER_ERROR)?;
                        Ok(ret)
                    }
                    Err(e) => {
                        http_error_bail!(INTERNAL_SERVER_ERROR, "Failed to open video file: {}", e)
                    }
                }
            } else {
                http_error_bail!(NOT_FOUND, "No matching video found");
            }
        } else {
            http_error_bail!(
                INTERNAL_SERVER_ERROR,
                "Failed to analyze video: {}",
                response.status()
            );
        }
    } else {
        http_error_bail!(BAD_REQUEST, "No video file provided");
    }
}

#[tokio::main]
async fn main() {
    tokio_fs::create_dir_all("./data/processed").await.unwrap();
    tokio_fs::create_dir_all("./data/temp").await.unwrap();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load video store at startup
    if let Err(e) = load_video_store().await {
        eprintln!("Failed to load video store: {}", e);
    }

    let state = AppState {
        processing_status: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/upload", post(upload_file))
        .route("/analyze", post(analyze_file))
        .route("/status/{session_id}", get(get_status))
        .route("/download/{session_id}", get(download_file))
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::max(250 * 1024 * 1024 * 1024))
        .layer(RequestBodyLimitLayer::new(250 * 1024 * 1024 * 1024)) // 250GB limit
        .with_state(state);

    let addr: SocketAddr = "0.0.0.0:8000".parse().unwrap();
    println!("Listening on {}", addr);
    Server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
